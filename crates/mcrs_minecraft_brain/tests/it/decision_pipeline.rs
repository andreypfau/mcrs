use std::collections::BTreeMap;

use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::ConfigError;
use mcrs_minecraft_brain::DecisionPipeline;
use mcrs_minecraft_brain::IssuedDecisions;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::Wait;
use mcrs_minecraft_brain::WorldSeed;

fn pipeline(delay: AiDelay) -> Result<DecisionPipeline, ConfigError> {
    let config = StageConfig::new(WorldSeed(1)).with_delay(2, delay)?;
    Ok(DecisionPipeline::new(&config))
}

fn ready(tick: u64) -> IssuedDecisions {
    IssuedDecisions {
        tick,
        tiles: BTreeMap::new(),
    }
}

fn ticks(due: Result<Vec<IssuedDecisions>, Wait>) -> Result<Vec<u64>, Wait> {
    due.map(|due| due.iter().map(|issued| issued.tick).collect())
}

#[test]
fn fixed_delay_applies_exactly_k_ticks_after_the_issue_and_waits_for_it() {
    let mut pipeline = pipeline(AiDelay::Fixed(2)).unwrap();
    pipeline.issue(10);
    pipeline.ready(ready(10));
    pipeline.issue(11);
    assert_eq!(ticks(pipeline.take_due(11)), Ok(vec![]));
    assert_eq!(ticks(pipeline.take_due(12)), Ok(vec![10]));
    assert_eq!(
        ticks(pipeline.take_due(13)),
        Err(Wait { issue_tick: 11 }),
        "the tick of 11 + 2 waits for the work of 11"
    );
    pipeline.ready(ready(11));
    assert_eq!(ticks(pipeline.take_due(13)), Ok(vec![11]));
}

#[test]
fn at_least_one_applies_when_ready_and_waits_only_at_max_lag() {
    let mut pipeline = pipeline(AiDelay::AtLeast(1)).unwrap();
    pipeline.issue(10);
    pipeline.issue(11);
    pipeline.ready(ready(11));
    assert_eq!(
        ticks(pipeline.take_due(11)),
        Ok(vec![]),
        "a later issue tick never applies before an earlier one that is not ready"
    );
    assert_eq!(ticks(pipeline.take_due(12)), Err(Wait { issue_tick: 10 }));
    pipeline.ready(ready(10));
    assert_eq!(ticks(pipeline.take_due(12)), Ok(vec![10, 11]));

    pipeline.issue(12);
    pipeline.ready(ready(12));
    assert_eq!(
        ticks(pipeline.take_due(13)),
        Ok(vec![12]),
        "a ready decision applies one tick after its issue"
    );
}

#[test]
fn a_delay_outside_max_lag_is_rejected() {
    let config = StageConfig::new(WorldSeed(1));
    assert!(config.with_delay(2, AiDelay::Fixed(3)).is_err());
    assert!(config.with_delay(2, AiDelay::Fixed(0)).is_err());
    assert!(config.with_delay(0, AiDelay::AtLeast(1)).is_err());
}
