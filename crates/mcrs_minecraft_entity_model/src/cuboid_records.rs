use crate::baked_geometry::{BakedCube, BakedGeometry};
use crate::explicit_face_record::{EXPLICIT_FACE_MIRROR, ExplicitFaceRecord};
use crate::gpu_record::{CuboidRecord, RecordError};

/// The records of the cubes of one model in the `cuboids` buffer: one [`CuboidRecord`] for each
/// cube, in the order of [`BakedGeometry::cubes`], then the explicit faces.
///
/// The explicit face `k` of the model is at the 32-byte slot `cuboids.len() + 2 · k` after the
/// first cuboid of the model.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct CuboidRecords {
    pub cuboids: Vec<CuboidRecord>,
    pub explicit_faces: Vec<ExplicitFaceRecord>,
}

impl CuboidRecords {
    /// The records of `model`. A cube with a rotation of its own, a per-face UV, or a box UV
    /// or an inflate that the 32-byte record cannot hold gets explicit faces.
    pub fn new(model: &BakedGeometry) -> Result<Self, RecordError> {
        let mut records = Self {
            cuboids: Vec::with_capacity(model.cubes.len()),
            explicit_faces: Vec::new(),
        };
        for cube in &model.cubes {
            let record = match CuboidRecord::try_from(cube) {
                Ok(record) => record,
                Err(RecordError::Bone) => return Err(RecordError::Bone),
                Err(_) => records.push_explicit(cube)?,
            };
            records.cuboids.push(record);
        }
        Ok(records)
    }

    /// The number of 32-byte slots that the records take.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.cuboids.len() + 2 * self.explicit_faces.len()
    }

    /// Appends the records to the bytes of a `cuboids` buffer.
    pub fn extend_bytes(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(bytemuck::cast_slice(&self.cuboids));
        buffer.extend_from_slice(bytemuck::cast_slice(&self.explicit_faces));
    }

    fn push_explicit(&mut self, cube: &BakedCube) -> Result<CuboidRecord, RecordError> {
        let bone = u8::try_from(cube.bone).map_err(|_| RecordError::Bone)?;
        let first_face = u32::try_from(self.explicit_faces.len()).unwrap_or(u32::MAX);
        let mirror = cube.shape.box_uv.is_some_and(|box_uv| box_uv.mirror);
        let flags = if mirror { EXPLICIT_FACE_MIRROR } else { 0 };
        let mut mask = 0;
        for face in &cube.faces {
            mask |= face.face.mask_bit();
            self.explicit_faces
                .push(ExplicitFaceRecord::new(face, bone, flags));
        }
        Ok(CuboidRecord::new_explicit(first_face, mask, bone))
    }
}
