use mcrs_minecraft_worldgen_feature::tree::FeatureSize;

/// The half-width the trunk needs at `y_offset` blocks above the origin of a
/// tree `tree_height` blocks tall.
pub fn size_at_height(size: &FeatureSize, tree_height: i32, y_offset: i32) -> i32 {
    match size {
        FeatureSize::TwoLayers {
            limit,
            lower_size,
            upper_size,
            ..
        } => {
            if y_offset < limit.0 {
                lower_size.0
            } else {
                upper_size.0
            }
        }
        FeatureSize::ThreeLayers {
            limit,
            upper_limit,
            lower_size,
            middle_size,
            upper_size,
            ..
        } => {
            if y_offset < limit.0 {
                lower_size.0
            } else if y_offset >= tree_height - upper_limit.0 {
                upper_size.0
            } else {
                middle_size.0
            }
        }
    }
}

/// How short a tree may be clipped to and still be placed. `None` forbids
/// clipping altogether.
pub fn min_clipped_height(size: &FeatureSize) -> Option<i32> {
    match size {
        FeatureSize::TwoLayers {
            min_clipped_height, ..
        }
        | FeatureSize::ThreeLayers {
            min_clipped_height, ..
        } => min_clipped_height.map(|height| height.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::codec::Bounded;

    #[test]
    fn the_layers_switch_at_their_limits() {
        let cases: [(FeatureSize, &[i32], Option<i32>); 2] = [
            (
                FeatureSize::TwoLayers {
                    limit: Bounded(3),
                    lower_size: Bounded(0),
                    upper_size: Bounded(2),
                    min_clipped_height: None,
                },
                &[0, 0, 0, 2, 2, 2],
                None,
            ),
            (
                FeatureSize::ThreeLayers {
                    limit: Bounded(2),
                    upper_limit: Bounded(3),
                    lower_size: Bounded(0),
                    middle_size: Bounded(1),
                    upper_size: Bounded(2),
                    min_clipped_height: Some(Bounded(4)),
                },
                &[0, 0, 1, 1, 1, 1, 1, 2, 2, 2],
                Some(4),
            ),
        ];
        for (size, widths, clipped) in cases {
            let drawn: Vec<i32> = (0..widths.len() as i32)
                .map(|y| size_at_height(&size, 10, y))
                .collect();
            assert_eq!(drawn, widths);
            assert_eq!(min_clipped_height(&size), clipped);
        }
    }
}
