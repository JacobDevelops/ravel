//! Failure codes are scoped to their crossing, not interchangeable statuses.

pub(super) const SNAPSHOT_MISMATCHES: usize = 2;
pub(super) const SNAPSHOT_ADMITTED: usize = 6;

#[derive(Clone, Copy)]
pub(super) enum SnapshotFailure {
    Generation,
    Lengths,
    State,
    Geometry,
    InconsistentGeometry,
    Palette,
    NullInput,
}

impl From<SnapshotFailure> for i64 {
    fn from(value: SnapshotFailure) -> Self {
        match value {
            SnapshotFailure::Generation => 1,
            SnapshotFailure::Lengths => 2,
            SnapshotFailure::State => 3,
            SnapshotFailure::Geometry => 4,
            SnapshotFailure::InconsistentGeometry => 5,
            SnapshotFailure::Palette => 6,
            SnapshotFailure::NullInput => 7,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum BakedFailure {
    Bounds,
    Decode,
    Emission,
    OracleHeader,
    LayerCount,
    OraclePayload,
    Allocation,
}

impl BakedFailure {
    pub(super) fn receipt(self) -> [i64; 6] {
        let code = match self {
            Self::Bounds => 1,
            Self::Decode => 2,
            Self::Emission => 3,
            Self::OracleHeader => 4,
            Self::LayerCount => 5,
            Self::OraclePayload => 6,
            Self::Allocation => 7,
        };
        [-1, code, 0, 0, 0, 0]
    }
}
