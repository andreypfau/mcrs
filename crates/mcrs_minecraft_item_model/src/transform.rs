use bevy_math::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// `T · Rl · S · Rr` as a record, or sixteen row-major floats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Transformation {
    Record {
        translation: [f32; 3],
        left_rotation: Quaternion,
        scale: [f32; 3],
        right_rotation: Quaternion,
    },
    Matrix([f32; 16]),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Quaternion {
    Components([f32; 4]),
    AxisAngle { angle: f32, axis: [f32; 3] },
}

impl Quaternion {
    pub fn quat(&self) -> Quat {
        match self {
            Self::Components([x, y, z, w]) => Quat::from_xyzw(*x, *y, *z, *w).normalize(),
            Self::AxisAngle { angle, axis } => {
                Quat::from_axis_angle(Vec3::from(*axis).normalize_or_zero(), *angle)
            }
        }
    }
}

impl Transformation {
    pub fn matrix(&self) -> Mat4 {
        match self {
            Self::Record {
                translation,
                left_rotation,
                scale,
                right_rotation,
            } => {
                Mat4::from_translation(Vec3::from(*translation))
                    * Mat4::from_quat(left_rotation.quat())
                    * Mat4::from_scale(Vec3::from(*scale))
                    * Mat4::from_quat(right_rotation.quat())
            }
            Self::Matrix(rows) => Mat4::from_cols_array(rows).transpose(),
        }
    }
}

/// `parent · child`, an absent child being the identity.
pub fn compose(parent: Mat4, child: Option<&Transformation>) -> Mat4 {
    match child {
        Some(child) => parent * child.matrix(),
        None => parent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_composes_translation_rotation_scale_and_rotation_in_that_order() {
        let record: Transformation = serde_json::from_str(
            r#"{"translation": [1, 2, 3], "left_rotation": {"angle": 1.5707964, "axis": [0, 1, 0]},
                "scale": [2, 2, 2], "right_rotation": [0, 0, 0, 1]}"#,
        )
        .unwrap();
        let point = record.matrix().transform_point3(Vec3::X);
        assert!(point.abs_diff_eq(Vec3::new(1.0, 2.0, 1.0), 1e-5), "{point}");
        let back: Transformation =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn a_matrix_reads_row_major() {
        let matrix: Transformation =
            serde_json::from_str("[1,0,0,5, 0,1,0,6, 0,0,1,7, 0,0,0,1]").unwrap();
        assert_eq!(
            matrix.matrix().transform_point3(Vec3::ZERO),
            Vec3::new(5.0, 6.0, 7.0)
        );
        assert!(matches!(matrix, Transformation::Matrix(_)));
    }

    #[test]
    fn a_component_quaternion_is_normalised_on_use_but_kept_as_written() {
        let q: Quaternion = serde_json::from_str("[0, 0, 0, 2]").unwrap();
        assert_eq!(q.quat(), Quat::IDENTITY);
        assert_eq!(serde_json::to_string(&q).unwrap(), "[0.0,0.0,0.0,2.0]");
    }
}
