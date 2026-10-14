use mcrs_minecraft_core::{ResourceLocation, VERSION};

use super::filter::Filter;
use super::metadata::{self, Metadata};
use super::{
    AvailablePack, Features, KnownPack, PackKind, PackSelection, PackStack, StackedPack, VANILLA,
};
use crate::diagnostics::{Diagnostic, Diagnostics};

struct Candidate<'a> {
    pack: &'a AvailablePack,
    metadata: Metadata,
}

fn disallowed(requested: &[ResourceLocation], features: &Features) -> Vec<ResourceLocation> {
    match &features.world {
        None => Vec::new(),
        Some(world) => requested
            .iter()
            .filter(|feature| !world.contains(feature))
            .cloned()
            .collect(),
    }
}

fn known_pack(pack: &AvailablePack) -> Option<KnownPack> {
    let id = match pack.kind {
        PackKind::Vanilla => "core",
        PackKind::Feature => pack.id.as_str(),
        PackKind::Directory | PackKind::Zip => return None,
    };
    Some(KnownPack {
        namespace: "minecraft".to_owned(),
        id: id.to_owned(),
        version: VERSION.id.clone(),
    })
}

fn candidates<'a>(
    available: &'a [AvailablePack],
    features: &Features,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Candidate<'a>> {
    let mut out: Vec<Candidate<'a>> = Vec::new();
    for pack in available {
        if out.iter().any(|seen| seen.pack.id == pack.id) {
            continue;
        }
        let metadata = match (pack.kind, &pack.metadata) {
            (PackKind::Vanilla, _) => Ok(Metadata {
                features: features.vanilla.clone(),
                filter: None,
                overlays: Vec::new(),
            }),
            (_, None) => Err("pack.mcmeta is missing".to_owned()),
            (_, Some(bytes)) => metadata::read(&pack.id, bytes, &features.known, diagnostics),
        };
        match metadata {
            Ok(metadata) => out.push(Candidate { pack, metadata }),
            Err(reason) => diagnostics.push(Diagnostic::UnavailablePack {
                pack: pack.id.clone(),
                reason,
            }),
        }
    }
    out
}

fn is_chosen(chosen: &[&Candidate<'_>], id: &str) -> bool {
    chosen.iter().any(|candidate| candidate.pack.id == id)
}

fn find<'c, 'a>(candidates: &'c [Candidate<'a>], id: &str) -> Option<&'c Candidate<'a>> {
    candidates.iter().find(|c| c.pack.id == id)
}

fn enabled<'c, 'a>(
    selection: &PackSelection,
    candidates: &'c [Candidate<'a>],
    features: &Features,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<&'c Candidate<'a>> {
    let mut chosen: Vec<&Candidate<'_>> = Vec::new();
    for id in &selection.enabled {
        let Some(candidate) = find(candidates, id) else {
            diagnostics.push(Diagnostic::MissingPack { pack: id.clone() });
            continue;
        };
        if is_chosen(&chosen, id) {
            continue;
        }
        let missing = disallowed(&candidate.metadata.features, features);
        if !selection.disabled.contains(id) && !missing.is_empty() {
            diagnostics.push(Diagnostic::PackFeaturesDisabled {
                pack: id.clone(),
                features: missing,
            });
            continue;
        }
        chosen.push(candidate);
    }
    chosen
}

fn append_fresh<'c, 'a>(
    chosen: &mut Vec<&'c Candidate<'a>>,
    selection: &PackSelection,
    candidates: &'c [Candidate<'a>],
    features: &Features,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut fresh: Vec<&Candidate<'_>> = candidates
        .iter()
        .filter(|candidate| {
            let pack = candidate.pack;
            pack.kind != PackKind::Feature
                && !selection.enabled.contains(&pack.id)
                && !selection.disabled.contains(&pack.id)
        })
        .collect();
    fresh.sort_by(|a, b| a.pack.id.cmp(&b.pack.id));
    for candidate in fresh {
        let pack = candidate.pack.id.clone();
        let missing = disallowed(&candidate.metadata.features, features);
        if missing.is_empty() {
            chosen.push(candidate);
            diagnostics.push(Diagnostic::PackFound { pack });
        } else {
            diagnostics.push(Diagnostic::PackNotAppended {
                pack,
                features: missing,
            });
        }
    }
}

fn append_feature_packs<'c, 'a>(
    chosen: &mut Vec<&'c Candidate<'a>>,
    candidates: &'c [Candidate<'a>],
    features: &Features,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(world) = &features.world else {
        return;
    };
    let provided = |chosen: &[&Candidate<'_>], feature: &ResourceLocation| {
        chosen
            .iter()
            .any(|candidate| candidate.metadata.features.contains(feature))
    };
    if world.iter().all(|feature| provided(chosen, feature)) {
        return;
    }
    let mut feature_packs: Vec<&Candidate<'_>> = candidates
        .iter()
        .filter(|candidate| {
            candidate.pack.kind == PackKind::Feature
                && !is_chosen(chosen, &candidate.pack.id)
                && disallowed(&candidate.metadata.features, features).is_empty()
        })
        .collect();
    feature_packs.sort_by(|a, b| a.pack.id.cmp(&b.pack.id));
    for candidate in feature_packs {
        chosen.push(candidate);
        diagnostics.push(Diagnostic::FeaturePackAppended {
            pack: candidate.pack.id.clone(),
        });
    }
}

fn stacked(chosen: Vec<&Candidate<'_>>, diagnostics: &mut Vec<Diagnostic>) -> Vec<StackedPack> {
    let mut packs = Vec::with_capacity(chosen.len());
    for candidate in chosen {
        let pack = candidate.pack;
        let filter = candidate.metadata.filter.as_ref().and_then(|section| {
            Filter::compile(section)
                .map_err(|reason| {
                    diagnostics.push(Diagnostic::FilterIgnored {
                        pack: pack.id.clone(),
                        reason,
                    })
                })
                .ok()
        });
        packs.push(StackedPack {
            id: pack.id.clone(),
            kind: pack.kind,
            known_pack: known_pack(pack),
            features: candidate.metadata.features.clone(),
            overlays: candidate.metadata.overlays.clone(),
            filter,
        });
    }
    packs
}

pub fn pack_stack(
    selection: &PackSelection,
    available: &[AvailablePack],
    features: &Features,
    safe_mode: bool,
) -> (PackStack, Diagnostics) {
    let mut diagnostics = Vec::new();
    let candidates = candidates(available, features, &mut diagnostics);
    let mut chosen: Vec<&Candidate<'_>> = Vec::new();

    if safe_mode {
        chosen.extend(find(&candidates, VANILLA));
    } else {
        chosen = enabled(selection, &candidates, features, &mut diagnostics);
        append_fresh(
            &mut chosen,
            selection,
            &candidates,
            features,
            &mut diagnostics,
        );
        append_feature_packs(&mut chosen, &candidates, features, &mut diagnostics);
        if chosen.is_empty() {
            chosen.extend(find(&candidates, VANILLA));
            diagnostics.push(Diagnostic::EmptyStack);
        }
    }

    let mut enabled_features: Vec<ResourceLocation> = chosen
        .iter()
        .flat_map(|candidate| candidate.metadata.features.iter().cloned())
        .chain(features.world.iter().flatten().cloned())
        .collect();
    enabled_features.sort();
    enabled_features.dedup();

    let packs = stacked(chosen, &mut diagnostics);
    let mut out = Diagnostics::default();
    out.extend(diagnostics);
    (
        PackStack {
            packs,
            enabled_features,
        },
        out,
    )
}
