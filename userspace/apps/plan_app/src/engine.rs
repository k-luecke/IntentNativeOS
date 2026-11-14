// userspace/apps/plan_app/src/engine.rs
// INOS Native Calendar, Task Engine & Temporal Scheduler
// Zero Python - 100% Freestanding Rust (No Heap / No Dynamic Allocator Required)

use core::cmp::{max, min};

pub const MAX_EVENTS: usize = 16;
pub const MAX_TASKS: usize = 16;
pub const MAX_EXDATES: usize = 8;
pub const MAX_OCCURRENCES: usize = 32;
pub const MAX_TITLE_LEN: usize = 48;
pub const MAX_LOC_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanError {
    BufferTooSmall,
    EventOverflow,
    TaskOverflow,
    InvalidDate,
    ParseError,
    NotFound,
}

// =========================================================================
// 1. TEMPORAL ENGINE (Contract L-01)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,   // 1..=12
    pub day: u8,     // 1..=31
    pub hour: u8,    // 0..=23
    pub minute: u8,  // 0..=59
    pub second: u8,  // 0..=59
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Returns day of week: 0 = Sunday, 1 = Monday, ..., 6 = Saturday (Sakamoto's algorithm)
pub fn day_of_week(year: i32, month: u8, day: u8) -> u8 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = year;
    if month < 3 {
        y -= 1;
    }
    let m_idx = (month - 1) as usize;
    let d = (y + y / 4 - y / 100 + y / 400 + t[m_idx] + day as i32) % 7;
    d as u8
}

impl DateTime {
    pub const fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// Convert to Unix Epoch timestamp (seconds since 1970-01-01 00:00:00 UTC)
    pub fn to_unix_epoch(&self) -> i64 {
        let mut days: i64 = 0;
        if self.year >= 1970 {
            for y in 1970..self.year {
                days += if is_leap_year(y) { 366 } else { 365 };
            }
        } else {
            for y in self.year..1970 {
                days -= if is_leap_year(y) { 366 } else { 365 };
            }
        }

        for m in 1..self.month {
            days += days_in_month(self.year, m) as i64;
        }
        days += (self.day - 1) as i64;

        days * 86400 + (self.hour as i64) * 3600 + (self.minute as i64) * 60 + (self.second as i64)
    }

    /// Construct DateTime from Unix Epoch timestamp
    pub fn from_unix_epoch(mut epoch: i64) -> Self {
        let mut year = 1970;
        let second = (epoch % 60) as u8;
        epoch /= 60;
        let minute = (epoch % 60) as u8;
        epoch /= 60;
        let hour = (epoch % 24) as u8;
        let mut days = epoch / 24;

        if days >= 0 {
            loop {
                let d_year = if is_leap_year(year) { 366 } else { 365 };
                if days < d_year {
                    break;
                }
                days -= d_year;
                year += 1;
            }
        } else {
            loop {
                year -= 1;
                let d_year = if is_leap_year(year) { 366 } else { 365 };
                days += d_year;
                if days >= 0 {
                    break;
                }
            }
        }

        let mut month = 1;
        loop {
            let dim = days_in_month(year, month) as i64;
            if days < dim {
                break;
            }
            days -= dim;
            month += 1;
        }
        let day = (days + 1) as u8;

        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    pub fn add_days(&self, days: i64) -> Self {
        let epoch = self.to_unix_epoch() + days * 86400;
        Self::from_unix_epoch(epoch)
    }

    pub fn add_minutes(&self, mins: i64) -> Self {
        let epoch = self.to_unix_epoch() + mins * 60;
        Self::from_unix_epoch(epoch)
    }

    /// Format to iCalendar format e.g. 20261015T093000Z
    pub fn format_ical(&self, buf: &mut [u8]) -> usize {
        if buf.len() < 16 {
            return 0;
        }
        let y = self.year as u32;
        buf[0] = b'0' + ((y / 1000) % 10) as u8;
        buf[1] = b'0' + ((y / 100) % 10) as u8;
        buf[2] = b'0' + ((y / 10) % 10) as u8;
        buf[3] = b'0' + (y % 10) as u8;

        buf[4] = b'0' + (self.month / 10);
        buf[5] = b'0' + (self.month % 10);

        buf[6] = b'0' + (self.day / 10);
        buf[7] = b'0' + (self.day % 10);

        buf[8] = b'T';

        buf[9] = b'0' + (self.hour / 10);
        buf[10] = b'0' + (self.hour % 10);

        buf[11] = b'0' + (self.minute / 10);
        buf[12] = b'0' + (self.minute % 10);

        buf[13] = b'0' + (self.second / 10);
        buf[14] = b'0' + (self.second % 10);

        buf[15] = b'Z';

        16
    }

    pub fn parse_ical(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 15 {
            return None;
        }
        let parse_num = |slice: &[u8]| -> Option<u32> {
            let mut val = 0u32;
            for &b in slice {
                if b < b'0' || b > b'9' {
                    return None;
                }
                val = val * 10 + (b - b'0') as u32;
            }
            Some(val)
        };

        let y = parse_num(&bytes[0..4])? as i32;
        let m = parse_num(&bytes[4..6])? as u8;
        let d = parse_num(&bytes[6..8])? as u8;
        let h = parse_num(&bytes[9..11])? as u8;
        let min = parse_num(&bytes[11..13])? as u8;
        let sec = parse_num(&bytes[13..15])? as u8;

        Some(Self::new(y, m, d, h, min, sec))
    }
}

// =========================================================================
// 2. RECURRENCE EVALUATOR (Contract L-02)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceFreq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

pub const DAY_MASK_SUNDAY: u8 = 1 << 0;
pub const DAY_MASK_MONDAY: u8 = 1 << 1;
pub const DAY_MASK_TUESDAY: u8 = 1 << 2;
pub const DAY_MASK_WEDNESDAY: u8 = 1 << 3;
pub const DAY_MASK_THURSDAY: u8 = 1 << 4;
pub const DAY_MASK_FRIDAY: u8 = 1 << 5;
pub const DAY_MASK_SATURDAY: u8 = 1 << 6;
pub const DAY_MASK_WEEKDAYS: u8 = DAY_MASK_MONDAY | DAY_MASK_TUESDAY | DAY_MASK_WEDNESDAY | DAY_MASK_THURSDAY | DAY_MASK_FRIDAY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RRule {
    pub freq: RecurrenceFreq,
    pub interval: u32,
    pub count: Option<u32>,
    pub until: Option<DateTime>,
    pub by_day_mask: u8,
    pub exdates: [DateTime; MAX_EXDATES],
    pub exdate_count: usize,
}

impl RRule {
    pub const fn new(freq: RecurrenceFreq) -> Self {
        Self {
            freq,
            interval: 1,
            count: None,
            until: None,
            by_day_mask: 0,
            exdates: [DateTime::new(1970, 1, 1, 0, 0, 0); MAX_EXDATES],
            exdate_count: 0,
        }
    }

    pub fn add_exdate(&mut self, dt: DateTime) -> bool {
        if self.exdate_count < MAX_EXDATES {
            self.exdates[self.exdate_count] = dt;
            self.exdate_count += 1;
            true
        } else {
            false
        }
    }

    pub fn expand(
        &self,
        dtstart: DateTime,
        window_start: DateTime,
        window_end: DateTime,
        out: &mut [DateTime],
    ) -> usize {
        let mut count_out = 0;
        let mut curr = dtstart;
        let mut generated_count: u32 = 0;
        let max_iterations = 1000;

        let interval = if self.interval == 0 { 1 } else { self.interval };

        for _ in 0..max_iterations {
            if let Some(limit) = self.count {
                if generated_count >= limit {
                    break;
                }
            }
            if let Some(until) = self.until {
                if curr > until {
                    break;
                }
            }

            let day_matches = if self.by_day_mask != 0 {
                let dow = day_of_week(curr.year, curr.month, curr.day);
                (self.by_day_mask & (1 << dow)) != 0
            } else {
                true
            };

            let mut is_excluded = false;
            for i in 0..self.exdate_count {
                let ex = &self.exdates[i];
                if ex.year == curr.year && ex.month == curr.month && ex.day == curr.day {
                    is_excluded = true;
                    break;
                }
            }

            if day_matches && !is_excluded {
                generated_count += 1;
                if curr >= window_start && curr <= window_end {
                    if count_out < out.len() {
                        out[count_out] = curr;
                        count_out += 1;
                    }
                }
            }

            if curr > window_end && self.until.map_or(true, |u| curr > u) {
                break;
            }

            // Step next
            match self.freq {
                RecurrenceFreq::Daily => {
                    curr = curr.add_days(interval as i64);
                }
                RecurrenceFreq::Weekly => {
                    if self.by_day_mask != 0 {
                        curr = curr.add_days(1);
                    } else {
                        curr = curr.add_days((interval * 7) as i64);
                    }
                }
                RecurrenceFreq::Monthly => {
                    let mut y = curr.year;
                    let mut m = curr.month as i32 + interval as i32;
                    while m > 12 {
                        m -= 12;
                        y += 1;
                    }
                    let m_u8 = m as u8;
                    let max_d = days_in_month(y, m_u8);
                    let d = min(curr.day, max_d);
                    curr = DateTime::new(y, m_u8, d, curr.hour, curr.minute, curr.second);
                }
                RecurrenceFreq::Yearly => {
                    let y = curr.year + interval as i32;
                    let max_d = days_in_month(y, curr.month);
                    let d = min(curr.day, max_d);
                    curr = DateTime::new(y, curr.month, d, curr.hour, curr.minute, curr.second);
                }
            }
        }

        count_out
    }
}

// =========================================================================
// 3. CALENDAR & CONFLICT DETECTION (Contract L-03)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarEvent {
    pub id: u32,
    pub title: [u8; MAX_TITLE_LEN],
    pub title_len: usize,
    pub start: DateTime,
    pub end: DateTime,
    pub location: [u8; MAX_LOC_LEN],
    pub loc_len: usize,
    pub priority: u8,
    pub rrule: Option<RRule>,
}

impl CalendarEvent {
    pub fn new(id: u32, title_str: &str, start: DateTime, end: DateTime) -> Self {
        let mut ev = Self {
            id,
            title: [0u8; MAX_TITLE_LEN],
            title_len: 0,
            start,
            end,
            location: [0u8; MAX_LOC_LEN],
            loc_len: 0,
            priority: 5,
            rrule: None,
        };
        ev.set_title(title_str);
        ev
    }

    pub fn set_title(&mut self, s: &str) {
        let b = s.as_bytes();
        let len = min(b.len(), MAX_TITLE_LEN);
        self.title[..len].copy_from_slice(&b[..len]);
        self.title_len = len;
    }

    pub fn set_location(&mut self, s: &str) {
        let b = s.as_bytes();
        let len = min(b.len(), MAX_LOC_LEN);
        self.location[..len].copy_from_slice(&b[..len]);
        self.loc_len = len;
    }

    pub fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
    }

    pub fn location_str(&self) -> &str {
        core::str::from_utf8(&self.location[..self.loc_len]).unwrap_or("")
    }
}

pub fn has_overlap(a_start: DateTime, a_end: DateTime, b_start: DateTime, b_end: DateTime) -> bool {
    let a_s = a_start.to_unix_epoch();
    let a_e = a_end.to_unix_epoch();
    let b_s = b_start.to_unix_epoch();
    let b_e = b_end.to_unix_epoch();
    max(a_s, b_s) < min(a_e, b_e)
}

pub fn detect_event_conflicts(
    events: &[CalendarEvent],
    conflicts_out: &mut [(u32, u32)],
) -> usize {
    let mut count = 0;
    for i in 0..events.len() {
        for j in (i + 1)..events.len() {
            if has_overlap(events[i].start, events[i].end, events[j].start, events[j].end) {
                if count < conflicts_out.len() {
                    conflicts_out[count] = (events[i].id, events[j].id);
                    count += 1;
                }
            }
        }
    }
    count
}

pub fn find_free_slot(
    events: &[CalendarEvent],
    window_start: DateTime,
    window_end: DateTime,
    duration_mins: i64,
) -> Option<(DateTime, DateTime)> {
    let mut cursor = window_start;
    let window_end_epoch = window_end.to_unix_epoch();

    while cursor.to_unix_epoch() + duration_mins * 60 <= window_end_epoch {
        let slot_end = cursor.add_minutes(duration_mins);
        let mut collision = false;
        let mut next_cursor = cursor.add_minutes(1);

        for ev in events {
            if has_overlap(cursor, slot_end, ev.start, ev.end) {
                collision = true;
                if ev.end > next_cursor {
                    next_cursor = ev.end;
                }
            }
        }

        if !collision {
            return Some((cursor, slot_end));
        }

        cursor = next_cursor;
    }

    None
}

// =========================================================================
// 4. TASK ENGINE & HIERARCHICAL SUBTASKS (Contract L-04)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanTask {
    pub id: u32,
    pub parent_id: Option<u32>,
    pub title: [u8; MAX_TITLE_LEN],
    pub title_len: usize,
    pub completed: bool,
    pub priority: u8,
    pub due: Option<DateTime>,
}

impl PlanTask {
    pub fn new(id: u32, title_str: &str) -> Self {
        let mut t = Self {
            id,
            parent_id: None,
            title: [0u8; MAX_TITLE_LEN],
            title_len: 0,
            completed: false,
            priority: 3,
            due: None,
        };
        t.set_title(title_str);
        t
    }

    pub fn set_title(&mut self, s: &str) {
        let b = s.as_bytes();
        let len = min(b.len(), MAX_TITLE_LEN);
        self.title[..len].copy_from_slice(&b[..len]);
        self.title_len = len;
    }

    pub fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TaskManager {
    pub tasks: [PlanTask; MAX_TASKS],
    pub count: usize,
}

impl TaskManager {
    pub const fn new() -> Self {
        Self {
            tasks: [PlanTask {
                id: 0,
                parent_id: None,
                title: [0u8; MAX_TITLE_LEN],
                title_len: 0,
                completed: false,
                priority: 3,
                due: None,
            }; MAX_TASKS],
            count: 0,
        }
    }

    pub fn add_task(&mut self, task: PlanTask) -> Result<(), PlanError> {
        if self.count >= MAX_TASKS {
            return Err(PlanError::TaskOverflow);
        }
        self.tasks[self.count] = task;
        self.count += 1;
        Ok(())
    }

    pub fn mark_completed(&mut self, id: u32, completed: bool) {
        for i in 0..self.count {
            if self.tasks[i].id == id {
                self.tasks[i].completed = completed;
                break;
            }
        }
    }

    pub fn subtask_progress(&self, parent_id: u32) -> (usize, usize, u32) {
        let mut total = 0;
        let mut completed = 0;
        for i in 0..self.count {
            if self.tasks[i].parent_id == Some(parent_id) {
                total += 1;
                if self.tasks[i].completed {
                    completed += 1;
                }
            }
        }
        if total == 0 {
            for i in 0..self.count {
                if self.tasks[i].id == parent_id {
                    return if self.tasks[i].completed { (1, 1, 100) } else { (0, 1, 0) };
                }
            }
            return (0, 0, 0);
        }
        let pct = ((completed * 100) / total) as u32;
        (completed, total, pct)
    }

    pub fn is_parent_completed(&self, parent_id: u32) -> bool {
        let mut has_subtasks = false;
        for i in 0..self.count {
            if self.tasks[i].parent_id == Some(parent_id) {
                has_subtasks = true;
                if !self.tasks[i].completed {
                    return false;
                }
            }
        }
        if !has_subtasks {
            for i in 0..self.count {
                if self.tasks[i].id == parent_id {
                    return self.tasks[i].completed;
                }
            }
            return false;
        }
        true
    }
}

// =========================================================================
// 5. RFC 5545 iCALENDAR (.ics) EXPORT & IMPORT (Contract L-05)
// =========================================================================

#[derive(Debug, Clone, Copy)]
pub struct CalendarDoc {
    pub events: [CalendarEvent; MAX_EVENTS],
    pub event_count: usize,
    pub task_mgr: TaskManager,
}

impl CalendarDoc {
    pub const fn new() -> Self {
        Self {
            events: [CalendarEvent {
                id: 0,
                title: [0u8; MAX_TITLE_LEN],
                title_len: 0,
                start: DateTime::new(1970, 1, 1, 0, 0, 0),
                end: DateTime::new(1970, 1, 1, 0, 0, 0),
                location: [0u8; MAX_LOC_LEN],
                loc_len: 0,
                priority: 5,
                rrule: None,
            }; MAX_EVENTS],
            event_count: 0,
            task_mgr: TaskManager::new(),
        }
    }

    pub fn add_event(&mut self, ev: CalendarEvent) -> Result<(), PlanError> {
        if self.event_count >= MAX_EVENTS {
            return Err(PlanError::EventOverflow);
        }
        self.events[self.event_count] = ev;
        self.event_count += 1;
        Ok(())
    }

    pub fn export_ics(&self, out: &mut [u8]) -> Result<usize, PlanError> {
        let mut pos = 0;

        let append = |dst: &mut [u8], p: &mut usize, src: &[u8]| -> Result<(), PlanError> {
            if *p + src.len() > dst.len() {
                return Err(PlanError::BufferTooSmall);
            }
            dst[*p..*p + src.len()].copy_from_slice(src);
            *p += src.len();
            Ok(())
        };

        append(out, &mut pos, b"BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//INOS//Plan 1.0//EN\r\nCALSCALE:GREGORIAN\r\n")?;

        for i in 0..self.event_count {
            let ev = &self.events[i];
            append(out, &mut pos, b"BEGIN:VEVENT\r\nUID:event-")?;
            
            // Format ID
            let mut id_buf = [0u8; 16];
            let id_len = format_u32(ev.id, &mut id_buf);
            append(out, &mut pos, &id_buf[..id_len])?;
            append(out, &mut pos, b"@inos.local\r\nSUMMARY:")?;
            append(out, &mut pos, &ev.title[..ev.title_len])?;
            append(out, &mut pos, b"\r\nDTSTART:")?;

            let mut dt_buf = [0u8; 16];
            ev.start.format_ical(&mut dt_buf);
            append(out, &mut pos, &dt_buf)?;
            append(out, &mut pos, b"\r\nDTEND:")?;

            ev.end.format_ical(&mut dt_buf);
            append(out, &mut pos, &dt_buf)?;
            append(out, &mut pos, b"\r\n")?;

            if ev.loc_len > 0 {
                append(out, &mut pos, b"LOCATION:")?;
                append(out, &mut pos, &ev.location[..ev.loc_len])?;
                append(out, &mut pos, b"\r\n")?;
            }

            append(out, &mut pos, b"PRIORITY:")?;
            let p_len = format_u32(ev.priority as u32, &mut id_buf);
            append(out, &mut pos, &id_buf[..p_len])?;
            append(out, &mut pos, b"\r\n")?;

            if let Some(rrule) = &ev.rrule {
                append(out, &mut pos, b"RRULE:FREQ=")?;
                match rrule.freq {
                    RecurrenceFreq::Daily => append(out, &mut pos, b"DAILY")?,
                    RecurrenceFreq::Weekly => append(out, &mut pos, b"WEEKLY")?,
                    RecurrenceFreq::Monthly => append(out, &mut pos, b"MONTHLY")?,
                    RecurrenceFreq::Yearly => append(out, &mut pos, b"YEARLY")?,
                }
                append(out, &mut pos, b";INTERVAL=")?;
                let iv_len = format_u32(rrule.interval, &mut id_buf);
                append(out, &mut pos, &id_buf[..iv_len])?;
                if let Some(c) = rrule.count {
                    append(out, &mut pos, b";COUNT=")?;
                    let c_len = format_u32(c, &mut id_buf);
                    append(out, &mut pos, &id_buf[..c_len])?;
                }
                append(out, &mut pos, b"\r\n")?;
            }

            append(out, &mut pos, b"END:VEVENT\r\n")?;
        }

        for i in 0..self.task_mgr.count {
            let t = &self.task_mgr.tasks[i];
            append(out, &mut pos, b"BEGIN:VTODO\r\nUID:todo-")?;
            let mut id_buf = [0u8; 16];
            let id_len = format_u32(t.id, &mut id_buf);
            append(out, &mut pos, &id_buf[..id_len])?;
            append(out, &mut pos, b"@inos.local\r\nSUMMARY:")?;
            append(out, &mut pos, &t.title[..t.title_len])?;
            append(out, &mut pos, b"\r\nSTATUS:")?;
            if t.completed {
                append(out, &mut pos, b"COMPLETED\r\n")?;
            } else {
                append(out, &mut pos, b"NEEDS-ACTION\r\n")?;
            }

            if let Some(pid) = t.parent_id {
                append(out, &mut pos, b"RELATED-TO:todo-")?;
                let pid_len = format_u32(pid, &mut id_buf);
                append(out, &mut pos, &id_buf[..pid_len])?;
                append(out, &mut pos, b"@inos.local\r\n")?;
            }

            append(out, &mut pos, b"END:VTODO\r\n")?;
        }

        append(out, &mut pos, b"END:VCALENDAR\r\n")?;
        Ok(pos)
    }

    pub fn import_ics(&mut self, ics: &[u8]) -> Result<(), PlanError> {
        let mut in_event = false;
        let mut in_todo = false;

        let mut ev_id = 0u32;
        let mut ev_title = [0u8; MAX_TITLE_LEN];
        let mut ev_title_len = 0;
        let mut ev_start = DateTime::new(2026, 1, 1, 0, 0, 0);
        let mut ev_end = DateTime::new(2026, 1, 1, 1, 0, 0);
        let mut ev_loc = [0u8; MAX_LOC_LEN];
        let mut ev_loc_len = 0;
        let mut ev_priority = 5u8;

        let mut td_id = 0u32;
        let mut td_title = [0u8; MAX_TITLE_LEN];
        let mut td_title_len = 0;
        let mut td_completed = false;
        let mut td_priority = 5u8;
        let mut td_due = None;
        let mut td_parent = None;

        let mut line_start = 0;
        while line_start < ics.len() {
            let mut line_end = line_start;
            while line_end < ics.len() && ics[line_end] != b'\n' {
                line_end += 1;
            }
            let mut trimmed_end = line_end;
            if trimmed_end > line_start && ics[trimmed_end - 1] == b'\r' {
                trimmed_end -= 1;
            }
            let line = &ics[line_start..trimmed_end];
            line_start = line_end + 1;

            if line == b"BEGIN:VEVENT" {
                in_event = true;
                ev_id += 1;
                ev_title_len = 0;
                ev_loc_len = 0;
                ev_priority = 5;
            } else if line == b"END:VEVENT" {
                if in_event {
                    let mut ev = CalendarEvent::new(ev_id, "", ev_start, ev_end);
                    ev.title = ev_title;
                    ev.title_len = ev_title_len;
                    ev.location = ev_loc;
                    ev.loc_len = ev_loc_len;
                    ev.priority = ev_priority;
                    let _ = self.add_event(ev);
                    in_event = false;
                }
            } else if line == b"BEGIN:VTODO" {
                in_todo = true;
                td_id += 1;
                td_title_len = 0;
                td_completed = false;
                td_priority = 5;
                td_due = None;
                td_parent = None;
            } else if line == b"END:VTODO" {
                if in_todo {
                    let mut t = PlanTask::new(td_id, "");
                    t.parent_id = td_parent;
                    t.title = td_title;
                    t.title_len = td_title_len;
                    t.completed = td_completed;
                    t.priority = td_priority;
                    t.due = td_due;
                    let _ = self.task_mgr.add_task(t);
                    in_todo = false;
                }
            } else if line.starts_with(b"SUMMARY:") {
                let val = &line[8..];
                let len = min(val.len(), MAX_TITLE_LEN);
                if in_event {
                    ev_title[..len].copy_from_slice(&val[..len]);
                    ev_title_len = len;
                } else if in_todo {
                    td_title[..len].copy_from_slice(&val[..len]);
                    td_title_len = len;
                }
            } else if line.starts_with(b"DTSTART:") {
                if in_event {
                    if let Some(dt) = DateTime::parse_ical(&line[8..]) {
                        ev_start = dt;
                    }
                }
            } else if line.starts_with(b"DTEND:") {
                if in_event {
                    if let Some(dt) = DateTime::parse_ical(&line[6..]) {
                        ev_end = dt;
                    }
                }
            } else if line.starts_with(b"LOCATION:") {
                if in_event {
                    let val = &line[9..];
                    let len = min(val.len(), MAX_LOC_LEN);
                    ev_loc[..len].copy_from_slice(&val[..len]);
                    ev_loc_len = len;
                }
            } else if line.starts_with(b"STATUS:") {
                if in_todo {
                    td_completed = &line[7..] == b"COMPLETED";
                }
            }
        }

        Ok(())
    }
}

pub fn format_u32(mut val: u32, buf: &mut [u8]) -> usize {
    if val == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return 1;
        }
        return 0;
    }
    let mut tmp = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        tmp[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    let len = min(i, buf.len());
    for j in 0..len {
        buf[j] = tmp[i - 1 - j];
    }
    len
}
