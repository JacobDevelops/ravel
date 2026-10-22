use jni::sys::jlong;
use ravel_renderer::baked::BakedMesh;

pub(super) struct Retained {
    pub(super) identity: [jlong; 7],
    pub(super) mesh: BakedMesh,
}

impl Retained {
    fn bytes(&self) -> usize {
        self.mesh.heap_bytes()
    }
}

pub(crate) struct Residency {
    next: jlong,
    pending: Vec<Retained>,
    resident: Vec<Retained>,
    budget: usize,
    evicted: jlong,
    peak: usize,
    budget_peak: usize,
    acknowledged: jlong,
    retired: jlong,
}

impl Residency {
    pub(crate) const fn new() -> Self {
        Self {
            next: 0,
            pending: Vec::new(),
            resident: Vec::new(),
            budget: 16 * 1024 * 1024,
            evicted: 0,
            peak: 0,
            budget_peak: 0,
            acknowledged: 0,
            retired: 0,
        }
    }

    fn reserve(&mut self) -> Option<()> {
        if self.pending.capacity() == 0 {
            self.pending.try_reserve_exact(64).ok()?;
        }
        if self.resident.capacity() == 0 {
            self.resident.try_reserve_exact(4096).ok()?;
        }
        Some(())
    }

    pub(super) fn busy(&self) -> bool {
        self.pending.len() >= 64
    }

    pub(super) fn publish(&mut self, mut value: Retained) -> Option<jlong> {
        if self.busy()
            || value.identity[0] <= 0
            || value.identity[2] <= 0
            || value.identity[2] != value.identity[3]
            || value.identity[4..].iter().any(|axis| axis & 15 != 0)
            || value.mesh.layers().iter().map(Vec::capacity).sum::<usize>() > super::MAX_QUADS
            || value.mesh.generation() != u64::try_from(value.identity[2]).ok()?
        {
            return None;
        }
        if self.pending.iter().any(|pending| {
            pending.identity[0] == value.identity[0] && pending.identity[4..] == value.identity[4..]
        }) {
            return None;
        }
        self.reserve()?;
        if value.bytes() > self.budget {
            return None;
        }
        while self.bytes().checked_add(value.bytes())? > self.budget
            || self.resident.len() + self.pending.len() >= 4096
        {
            if self.resident.is_empty() {
                return None;
            }
            let index = self
                .resident
                .iter()
                .enumerate()
                .max_by_key(|(_, entry)| {
                    entry.identity[4..]
                        .iter()
                        .zip(&value.identity[4..])
                        .map(|(left, right)| left.abs_diff(*right))
                        .sum::<u64>()
                })?
                .0;
            self.resident.remove(index);
            self.retired += 1;
            self.evicted += 1;
        }
        self.next = self.next.checked_add(1)?;
        value.identity[1] = self.next;
        self.pending.push(value);
        let bytes = self.bytes();
        self.peak = self.peak.max(bytes);
        self.budget_peak = self.budget_peak.max(bytes);
        Some(self.next)
    }

    pub(super) fn settle(&mut self, generation: jlong, key: jlong, accept: bool) -> bool {
        let Some(index) = self
            .pending
            .iter()
            .position(|value| value.identity[..2] == [generation, key])
        else {
            return false;
        };
        let mut pending = self.pending.remove(index);
        if accept {
            pending.mesh.release_uploaded_opaque();
            if let Some(index) = self.resident.iter().position(|value| {
                value.identity[0] == generation && value.identity[4..] == pending.identity[4..]
            }) {
                self.resident.remove(index);
                self.retired += 1;
            }
            let index = self
                .resident
                .partition_point(|entry| entry.identity[1] < key);
            self.resident.insert(index, pending);
            self.acknowledged += 1;
        } else {
            self.retired += 1;
        }
        true
    }

    pub(super) fn resort(
        &mut self,
        generation: jlong,
        key: jlong,
        camera: [f32; 3],
        output_words: usize,
    ) -> Option<&BakedMesh> {
        let value = self
            .resident
            .iter_mut()
            .chain(self.pending.iter_mut())
            .find(|value| value.identity[..2] == [generation, key])?;
        if output_words != 4 + value.mesh.layers()[3].len() * 28 {
            return None;
        }
        value
            .mesh
            .resort(u64::try_from(value.identity[2]).ok()?, camera, || false)
            .ok()?;
        Some(&value.mesh)
    }

    pub(crate) fn retire(&mut self) {
        self.retired += super::super::metric_long(self.pending.len() + self.resident.len());
        self.pending = Vec::new();
        self.resident = Vec::new();
    }

    fn bytes(&self) -> usize {
        let entries = (self.pending.capacity() + self.resident.capacity()) * size_of::<Retained>();
        entries
            + self
                .pending
                .iter()
                .chain(&self.resident)
                .map(Retained::bytes)
                .sum::<usize>()
    }

    pub(super) fn metrics(&self) -> [jlong; 12] {
        let identity = self.resident.last().map_or([0; 7], |value| value.identity);
        [
            super::super::metric_long(self.resident.iter().map(Retained::bytes).sum::<usize>()),
            super::super::metric_long(self.pending.iter().map(Retained::bytes).sum::<usize>()),
            super::super::metric_long(self.peak),
            self.acknowledged,
            self.retired,
            identity[0],
            identity[1],
            identity[2],
            identity[3],
            identity[4],
            identity[5],
            identity[6],
        ]
    }

    pub(super) fn configure(&mut self, distance: jlong) -> bool {
        if !(1..=32).contains(&distance) || self.bytes() != 0 {
            return false;
        }
        let diameter = usize::try_from(distance * 2 + 1).expect("bounded render distance");
        let budget = (diameter * diameter * 262_144).clamp(16 * 1024 * 1024, 256 * 1024 * 1024);
        if self.budget != budget {
            self.budget_peak = 0;
        }
        self.budget = budget;
        true
    }

    pub(super) fn contains(&self, generation: jlong, key: jlong) -> bool {
        self.resident
            .binary_search_by_key(&key, |value| value.identity[1])
            .is_ok_and(|index| self.resident[index].identity[0] == generation)
    }

    pub(super) fn dedupe(&self) -> [jlong; 2] {
        let released = self
            .resident
            .iter()
            .map(|value| value.mesh.released_bytes())
            .sum::<usize>();
        [
            super::super::metric_long(self.bytes() + released),
            super::super::metric_long(self.bytes()),
        ]
    }

    pub(super) fn bounds(&self) -> [jlong; 6] {
        [
            super::super::metric_long(self.budget),
            super::super::metric_long(self.resident.len()),
            super::super::metric_long(self.pending.len()),
            self.evicted,
            super::super::metric_long(self.bytes()),
            super::super::metric_long(self.budget_peak),
        ]
    }
}

impl Default for Residency {
    fn default() -> Self {
        Self::new()
    }
}
