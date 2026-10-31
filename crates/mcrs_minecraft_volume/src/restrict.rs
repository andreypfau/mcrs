use std::ops::{Deref, DerefMut};

use mcrs_minecraft_core::BoundingBox;
use mcrs_voxel::SectionPos;

use crate::access::{Access, Denied, Unavailable, Volume};
use crate::block::{BlockVolume, BlockVolumeMut};
use crate::bounds::ColumnLayout;

/// Which rights a holder keeps per section, on top of what the store itself grants.
pub trait AccessPolicy {
    fn access(&self, section: SectionPos) -> Access;
}

/// A borrowed volume whose rights are narrowed by a policy: each section's access is the
/// weaker of the two answers, so a restriction only ever takes rights away.
pub struct Restricted<V, P> {
    inner: V,
    policy: P,
}

impl<V, P> Restricted<V, P> {
    pub fn new(inner: V, policy: P) -> Self {
        Self { inner, policy }
    }
}

impl<V, P> Volume for Restricted<V, P>
where
    V: Deref,
    V::Target: Volume,
    P: AccessPolicy,
{
    fn bounds(&self) -> BoundingBox {
        self.inner.bounds()
    }

    fn access(&self, section: SectionPos) -> Access {
        self.inner.access(section).meet(self.policy.access(section))
    }

    fn column_layout(&self) -> Option<ColumnLayout> {
        self.inner.column_layout()
    }
}

impl<V, P> BlockVolume for Restricted<V, P>
where
    V: Deref,
    V::Target: BlockVolume,
    P: AccessPolicy,
{
    type Section<'a>
        = <V::Target as BlockVolume>::Section<'a>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> Result<Self::Section<'_>, Unavailable> {
        self.access(at).check_read()?;
        self.inner.section(at)
    }
}

impl<V, P> BlockVolumeMut for Restricted<V, P>
where
    V: DerefMut,
    V::Target: BlockVolumeMut,
    P: AccessPolicy,
{
    type SectionMut<'a>
        = <V::Target as BlockVolumeMut>::SectionMut<'a>
    where
        Self: 'a;

    fn section_mut(&mut self, at: SectionPos) -> Result<Self::SectionMut<'_>, Denied> {
        self.access(at).check_write()?;
        self.inner.section_mut(at)
    }
}
