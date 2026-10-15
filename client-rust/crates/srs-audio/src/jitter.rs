//! Jitter buffer: a faithful port of the C# port of speexdsp's `jitter.c`
//! (`Common/Audio/Utility/Speex/JitterBuffer.cs`), **including its
//! deviations from upstream** (they shape the audible behaviour):
//!
//! - `buffer_margin` is never set (always 0), `late_cutoff` is unused;
//! - when the buffer is full, the "discard the oldest" loop always evicts the
//!   last slot (`packets[i] != null || ...` is always true);
//! - `MaxLateRate` is 4 → `window_size` 1000, `subwindow_size` 333.
//!
//! Timestamps are in samples; SRS uses `packet_number * samples_per_packet`
//! and a `step` of 1920 (40 ms at 48 kHz).

const MAX_BUFFER_SIZE: usize = 200;
const MAX_BUFFERS: usize = 3;
const TOP_DELAY: usize = 40;
const MAX_TIMINGS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Missing,
    Insertion,
}

#[derive(Debug, Clone)]
pub struct Packet<T> {
    pub data: Option<T>,
    pub timestamp: i64,
    pub span: i64,
}

#[derive(Debug, Clone)]
struct TimingBuffer {
    filled: usize,
    curr_count: i32,
    timings: [i64; MAX_TIMINGS],
    counts: [i32; MAX_TIMINGS],
}

impl TimingBuffer {
    fn new() -> Self {
        Self {
            filled: 0,
            curr_count: 0,
            timings: [0; MAX_TIMINGS],
            counts: [0; MAX_TIMINGS],
        }
    }

    fn init(&mut self) {
        self.filled = 0;
        self.curr_count = 0;
    }

    fn add(&mut self, timing: i64) {
        if self.filled >= MAX_TIMINGS && timing >= self.timings[self.filled - 1] {
            self.curr_count += 1;
            return;
        }
        let mut pos = 0;
        while pos < self.filled && timing >= self.timings[pos] {
            pos += 1;
        }
        debug_assert!(pos <= self.filled && pos < MAX_TIMINGS);
        if pos < self.filled {
            let mut move_size = self.filled - pos;
            if self.filled == MAX_TIMINGS {
                move_size -= 1;
            }
            self.timings.copy_within(pos..pos + move_size, pos + 1);
            self.counts.copy_within(pos..pos + move_size, pos + 1);
        }
        self.timings[pos] = timing;
        self.counts[pos] = self.curr_count;
        self.curr_count += 1;
        if self.filled < MAX_TIMINGS {
            self.filled += 1;
        }
    }
}

#[derive(Debug)]
pub struct JitterBuffer<T> {
    pointer_timestamp: i64,
    last_returned_timestamp: i64,
    next_stop: i64,
    buffered: i64,
    packets: Vec<Option<Packet<T>>>,
    arrival: [i64; MAX_BUFFER_SIZE],
    delay_step: i64,
    concealment_size: i64,
    reset_state: bool,
    buffer_margin: i64,
    interp_requested: i64,
    auto_adjust: bool,
    tb: [TimingBuffer; MAX_BUFFERS],
    time_buffers: [usize; MAX_BUFFERS],
    window_size: i64,
    subwindow_size: i32,
    max_late_rate: i64,
    latency_tradeoff: i64,
    auto_tradeoff: i64,
    lost_count: i32,
}

fn round_down(x: i64, step: i64) -> i64 {
    ((if x < 0 { x - step + 1 } else { x }) / step) * step
}

impl<T> JitterBuffer<T> {
    pub fn new(step_size: i64) -> Self {
        let mut jb = Self {
            pointer_timestamp: 0,
            last_returned_timestamp: 0,
            next_stop: 0,
            buffered: 0,
            packets: (0..MAX_BUFFER_SIZE).map(|_| None).collect(),
            arrival: [0; MAX_BUFFER_SIZE],
            delay_step: step_size,
            concealment_size: step_size,
            reset_state: true,
            buffer_margin: 0,
            interp_requested: 0,
            auto_adjust: true,
            tb: [TimingBuffer::new(), TimingBuffer::new(), TimingBuffer::new()],
            time_buffers: [0, 1, 2],
            window_size: 0,
            subwindow_size: 0,
            max_late_rate: 0,
            latency_tradeoff: 0,
            auto_tradeoff: 0,
            lost_count: 0,
        };
        jb.set_max_late_rate(4);
        jb.reset();
        jb
    }

    pub fn reset(&mut self) {
        for p in &mut self.packets {
            *p = None;
        }
        self.pointer_timestamp = 0;
        self.next_stop = 0;
        self.reset_state = true;
        self.lost_count = 0;
        self.buffered = 0;
        self.auto_tradeoff = 32000;
        for (i, tb) in self.tb.iter_mut().enumerate() {
            *tb = TimingBuffer::new();
            self.time_buffers[i] = i;
        }
    }

    pub fn set_max_late_rate(&mut self, value: i64) {
        self.max_late_rate = value;
        self.window_size = 100 * TOP_DELAY as i64 / value;
        self.subwindow_size = (self.window_size / MAX_BUFFERS as i64) as i32;
    }

    pub fn pointer_timestamp(&self) -> i64 {
        self.pointer_timestamp
    }

    pub fn delay_step(&self) -> i64 {
        self.delay_step
    }

    #[allow(clippy::needless_range_loop)]
    fn compute_opt_delay(&mut self) -> i64 {
        let tot_count: i32 = self.tb.iter().map(|t| t.curr_count).sum();
        if tot_count == 0 {
            return 0;
        }
        let late_factor: f32 = if self.latency_tradeoff != 0 {
            self.latency_tradeoff as f32 * 100.0 / tot_count as f32
        } else {
            (self.auto_tradeoff * self.window_size) as f32 / tot_count as f32
        };
        let mut worst = 0i64;
        let mut best = 0i64;
        let mut late = 0i32;
        let mut best_cost = i32::MAX;
        let mut opt = 0i64;
        let mut pos = [0usize; MAX_BUFFERS];
        let mut penalty_taken = false;
        for i in 0..TOP_DELAY {
            let mut next: Option<usize> = None;
            let mut latest: i64 = i16::MAX as i64;
            for j in 0..MAX_BUFFERS {
                if pos[j] < self.tb[j].filled && self.tb[j].timings[pos[j]] < latest {
                    next = Some(j);
                    latest = self.tb[j].timings[pos[j]];
                }
            }
            match next {
                Some(j) => {
                    if i == 0 {
                        worst = latest;
                    }
                    best = latest;
                    latest = round_down(latest, self.delay_step);
                    pos[j] += 1;
                    let cost = (-latest as f32 + late_factor * late as f32) as i32;
                    if cost < best_cost {
                        best_cost = cost;
                        opt = latest;
                    }
                }
                None => break,
            }
            late += 1;
            if latest >= 0 && !penalty_taken {
                late += 4;
                penalty_taken = true;
            }
        }
        let delta_t = best - worst;
        self.auto_tradeoff = 1 + delta_t / TOP_DELAY as i64;
        if (tot_count as usize) < TOP_DELAY && opt > 0 {
            return 0;
        }
        opt
    }

    fn update_timings(&mut self, timing: i64) {
        let timing = timing.clamp(i16::MIN as i64 + 1, i16::MAX as i64);
        if self.tb[self.time_buffers[0]].curr_count >= self.subwindow_size {
            let tmp = self.time_buffers[MAX_BUFFERS - 1];
            for i in (1..MAX_BUFFERS).rev() {
                self.time_buffers[i] = self.time_buffers[i - 1];
            }
            self.time_buffers[0] = tmp;
            self.tb[self.time_buffers[0]].init();
        }
        self.tb[self.time_buffers[0]].add(timing);
    }

    fn shift_timings(&mut self, amount: i64) {
        for i in 0..MAX_BUFFERS {
            let tb = &mut self.tb[self.time_buffers[i]];
            for j in 0..tb.filled {
                tb.timings[j] += amount;
            }
        }
    }

    /// Insert one packet.
    pub fn put(&mut self, packet: Packet<T>) {
        let mut late = false;
        if !self.reset_state {
            for p in &mut self.packets {
                if let Some(pk) = p {
                    if pk.timestamp + pk.span <= self.pointer_timestamp {
                        *p = None;
                    }
                }
            }
            if packet.timestamp < self.next_stop {
                self.update_timings(packet.timestamp - self.next_stop - self.buffer_margin);
                late = true;
            }
        }
        if self.lost_count > 20 {
            self.reset();
        }
        if self.reset_state || packet.timestamp + packet.span + self.delay_step >= self.pointer_timestamp {
            let mut i = self
                .packets
                .iter()
                .position(Option::is_none)
                .unwrap_or(MAX_BUFFER_SIZE);
            if i == MAX_BUFFER_SIZE {
                // C# quirk: `packets[i] != null || ...` is always true once the
                // buffer is full, so the loop ends with i = last index.
                let mut earliest = self.packets[0].as_ref().map(|p| p.timestamp);
                i = 0;
                for j in 1..MAX_BUFFER_SIZE {
                    let tj = self.packets[j].as_ref().map(|p| p.timestamp);
                    if self.packets[i].is_some() || (tj.is_some() && earliest.is_some() && tj < earliest) {
                        earliest = tj;
                        i = j;
                    }
                }
                self.packets[i] = None;
            }
            self.arrival[i] = if self.reset_state || late {
                0
            } else {
                self.next_stop
            };
            self.packets[i] = Some(packet);
        }
    }

    /// Fetch the packet covering the next `desired_span` samples.
    pub fn get(&mut self, mut desired_span: i64) -> (Status, Packet<T>) {
        if self.reset_state {
            let mut found = false;
            let mut oldest = 0i64;
            for p in self.packets.iter().flatten() {
                if !found || p.timestamp < oldest {
                    oldest = p.timestamp;
                    found = true;
                }
            }
            if found {
                self.reset_state = false;
                self.pointer_timestamp = oldest;
                self.next_stop = oldest;
            } else {
                return (
                    Status::Missing,
                    Packet {
                        data: None,
                        timestamp: 0,
                        span: self.interp_requested,
                    },
                );
            }
        }

        self.last_returned_timestamp = self.pointer_timestamp;
        if self.interp_requested != 0 {
            let out = Packet {
                data: None,
                timestamp: self.pointer_timestamp,
                span: self.interp_requested,
            };
            self.pointer_timestamp += self.interp_requested;
            self.interp_requested = 0;
            self.buffered = out.span - desired_span;
            return (Status::Insertion, out);
        }

        let pt = self.pointer_timestamp;
        let exact = |p: &Packet<T>| p.timestamp == pt && p.timestamp + p.span >= pt + desired_span;
        let older_full = |p: &Packet<T>| p.timestamp <= pt && p.timestamp + p.span >= pt + desired_span;
        let older_part = |p: &Packet<T>| p.timestamp <= pt && p.timestamp + p.span > pt;

        let mut found = self.packets.iter().position(|p| p.as_ref().is_some_and(exact));
        if found.is_none() {
            found = self
                .packets
                .iter()
                .position(|p| p.as_ref().is_some_and(older_full));
        }
        if found.is_none() {
            found = self
                .packets
                .iter()
                .position(|p| p.as_ref().is_some_and(older_part));
        }
        if found.is_none() {
            let mut best: Option<(usize, i64, i64)> = None;
            for (i, p) in self.packets.iter().enumerate() {
                if let Some(p) = p {
                    if p.timestamp < pt + desired_span && p.timestamp >= pt {
                        let better = match best {
                            None => true,
                            Some((_, bts, bspan)) => {
                                p.timestamp < bts || (p.timestamp == bts && p.span > bspan)
                            }
                        };
                        if better {
                            best = Some((i, p.timestamp, p.span));
                        }
                    }
                }
            }
            found = best.map(|(i, _, _)| i);
        }

        if let Some(i) = found {
            self.lost_count = 0;
            let ts = self.packets[i].as_ref().map(|p| p.timestamp).unwrap_or(0);
            if self.arrival[i] != 0 {
                self.update_timings(ts - self.arrival[i] - self.buffer_margin);
            }
            let out = self.packets[i].take().expect("found packet");
            self.last_returned_timestamp = out.timestamp;
            self.pointer_timestamp = out.timestamp + out.span;
            self.buffered = out.span - desired_span.min(out.span);
            return (Status::Ok, out);
        }

        self.lost_count += 1;
        let opt = self.compute_opt_delay();
        if opt < 0 {
            self.shift_timings(-opt);
            let out = Packet {
                data: None,
                timestamp: self.pointer_timestamp,
                span: -opt,
            };
            self.buffered = out.span - desired_span;
            return (Status::Insertion, out);
        }
        desired_span = round_down(desired_span, self.concealment_size);
        let out = Packet {
            data: None,
            timestamp: self.pointer_timestamp,
            span: desired_span,
        };
        self.pointer_timestamp += out.span;
        self.buffered = out.span - desired_span;
        (Status::Missing, out)
    }

    fn update_delay_inner(&mut self) -> i64 {
        let opt = self.compute_opt_delay();
        if opt != 0 {
            self.shift_timings(-opt);
            self.pointer_timestamp += opt;
            if opt < 0 {
                self.interp_requested = -opt;
            }
        }
        opt
    }

    pub fn update_delay(&mut self) -> i64 {
        self.auto_adjust = false;
        self.update_delay_inner()
    }

    /// Call once per consumer tick (after `get`).
    pub fn tick(&mut self) {
        if self.auto_adjust {
            self.update_delay_inner();
        }
        if self.buffered >= 0 {
            self.next_stop = self.pointer_timestamp - self.buffered;
        } else {
            self.next_stop = self.pointer_timestamp;
            debug_assert!(
                self.buffered >= 0,
                "Jitter sees negative buffering ({})",
                self.buffered
            );
        }
    }

    pub fn remaining_span(&mut self, rem: i64) {
        if self.auto_adjust {
            self.update_delay_inner();
        }
        self.next_stop = self.pointer_timestamp - rem;
    }

    /// Any packet at or after the read pointer?
    pub fn available(&self) -> bool {
        self.packets
            .iter()
            .flatten()
            .any(|p| p.timestamp >= self.pointer_timestamp)
    }

    pub fn last_returned_timestamp(&self) -> i64 {
        self.last_returned_timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: i64 = 1920;

    fn pk(n: i64) -> Packet<u32> {
        Packet {
            data: Some(n as u32),
            timestamp: n * STEP,
            span: STEP,
        }
    }

    #[test]
    fn empty_buffer_reports_missing() {
        let mut jb: JitterBuffer<u32> = JitterBuffer::new(STEP);
        assert!(!jb.available());
        let (st, _) = jb.get(STEP);
        assert_eq!(st, Status::Missing);
    }

    #[test]
    fn in_order_packets_come_back_in_order() {
        let mut jb = JitterBuffer::new(STEP);
        for n in 1..=5 {
            jb.put(pk(n));
        }
        assert!(jb.available());
        for n in 1..=5 {
            let (st, p) = jb.get(STEP);
            assert_eq!(st, Status::Ok);
            assert_eq!(p.data, Some(n as u32));
            jb.tick();
        }
        let (st, _) = jb.get(STEP);
        assert_eq!(st, Status::Missing);
    }

    #[test]
    fn reordered_packets_are_sorted_and_gaps_are_concealed() {
        let mut jb = JitterBuffer::new(STEP);
        jb.put(pk(2));
        jb.put(pk(1));
        jb.put(pk(4));
        let (st, p) = jb.get(STEP);
        assert_eq!((st, p.data), (Status::Ok, Some(1)));
        jb.tick();
        let (st, p) = jb.get(STEP);
        assert_eq!((st, p.data), (Status::Ok, Some(2)));
        jb.tick();
        let (st, p) = jb.get(STEP);
        assert_eq!(st, Status::Missing, "packet 3 was lost");
        assert_eq!(p.span, STEP);
        jb.tick();
        let (st, p) = jb.get(STEP);
        assert_eq!((st, p.data), (Status::Ok, Some(4)));
    }

    #[test]
    fn hopelessly_late_packet_is_dropped() {
        let mut jb = JitterBuffer::new(STEP);
        for n in 10..=12 {
            jb.put(pk(n));
        }
        for _ in 0..3 {
            jb.get(STEP);
            jb.tick();
        }
        jb.put(pk(1));
        assert!(!jb.available());
    }

    #[test]
    fn consumer_resyncs_after_long_silence() {
        let mut jb = JitterBuffer::new(STEP);
        jb.put(pk(1));
        jb.get(STEP);
        jb.tick();
        for _ in 0..25 {
            let (st, _) = jb.get(STEP);
            assert_eq!(st, Status::Missing);
            jb.tick();
        }
        // lost_count > 20 -> the next put resets and resyncs on that packet.
        jb.put(pk(500));
        let (st, p) = jb.get(STEP);
        assert_eq!((st, p.data), (Status::Ok, Some(500)));
    }
}
