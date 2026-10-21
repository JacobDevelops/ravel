use jni::sys::jlong;

pub(super) struct Retained {
    pub(super) identity: [jlong; 7],
    pub(super) source: Vec<u8>,
    pub(super) emitted: Vec<i32>,
}

impl Retained {
    fn bytes(&self) -> usize {
        self.source.capacity() + self.emitted.capacity() * 4
    }
}

#[derive(Default)]
pub(crate) struct Residency {
    next: jlong,
    pending: Option<Retained>,
    resident: Option<Retained>,
    peak: usize,
    acknowledged: jlong,
    retired: jlong,
}

impl Residency {
    pub(crate) const fn new() -> Self {
        Self {
            next: 0,
            pending: None,
            resident: None,
            peak: 0,
            acknowledged: 0,
            retired: 0,
        }
    }

    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn publish(&mut self, mut value: Retained) -> Option<jlong> {
        if self.busy()
            || value.identity[0] <= 0
            || value.identity[2] <= 0
            || value.identity[2] != value.identity[3]
            || value.identity[4..].iter().any(|axis| axis & 15 != 0)
            || value.source.capacity() > super::INPUT_BOUND
            || value.emitted.capacity() > 4 + super::MAX_QUADS * 28
        {
            return None;
        }
        self.next = self.next.checked_add(1)?;
        value.identity[1] = self.next;
        self.pending = Some(value);
        self.peak = self.peak.max(self.bytes());
        Some(self.next)
    }

    pub(super) fn settle(&mut self, generation: jlong, key: jlong, accept: bool) -> bool {
        if self
            .pending
            .as_ref()
            .is_none_or(|value| value.identity[..2] != [generation, key])
        {
            return false;
        }
        let pending = self.pending.take();
        if accept {
            self.retired += jlong::from(self.resident.is_some());
            self.resident = pending;
            self.acknowledged += 1;
        } else {
            self.retired += 1;
        }
        true
    }

    pub(crate) fn retire(&mut self) {
        self.retired += jlong::from(self.pending.take().is_some());
        self.retired += jlong::from(self.resident.take().is_some());
    }

    fn bytes(&self) -> usize {
        self.pending.as_ref().map_or(0, Retained::bytes)
            + self.resident.as_ref().map_or(0, Retained::bytes)
    }

    pub(super) fn metrics(&self) -> [jlong; 12] {
        let identity = self
            .resident
            .as_ref()
            .map_or([0; 7], |value| value.identity);
        [
            super::super::metric_long(self.resident.as_ref().map_or(0, Retained::bytes)),
            super::super::metric_long(self.pending.as_ref().map_or(0, Retained::bytes)),
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
}
