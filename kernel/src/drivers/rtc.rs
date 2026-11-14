// kernel/src/drivers/rtc.rs - CMOS Real-Time Clock (RTC) Driver
// Provides provisional wall-clock time bootstrapping.
// NOTE: CMOS RTC is treated strictly as an untrusted initial provisional clock source.
// Monotonic time is provided separately via RDTSC / system timer ticks.

use crate::arch::x86_64::cpu::{inb, outb, rdtsc};

const CMOS_ADDR_PORT: u16 = 0x70;
const CMOS_DATA_PORT: u16 = 0x71;

const RTC_REG_SECONDS: u8 = 0x00;
const RTC_REG_MINUTES: u8 = 0x02;
const RTC_REG_HOURS: u8 = 0x04;
const RTC_REG_DAY: u8 = 0x07;
const RTC_REG_MONTH: u8 = 0x08;
const RTC_REG_YEAR: u8 = 0x09;
const RTC_REG_STATUS_A: u8 = 0x0A;
const RTC_REG_STATUS_B: u8 = 0x0B;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub unix_timestamp: u64,
}

pub struct RtcDriver {
    initial_boot_epoch: u64,
    initial_rdtsc: u64,
    is_provisional_valid: bool,
}

pub static mut GLOBAL_RTC: RtcDriver = RtcDriver {
    initial_boot_epoch: 0,
    initial_rdtsc: 0,
    is_provisional_valid: false,
};

impl RtcDriver {
    pub fn init(&mut self) {
        if let Some(dt) = self.read_datetime() {
            self.initial_boot_epoch = dt.unix_timestamp;
            self.initial_rdtsc = unsafe { rdtsc() };
            self.is_provisional_valid = true;
        } else {
            self.is_provisional_valid = false;
        }
    }

    /// Reads CMOS RTC with update-in-progress (UIP) loop protection.
    pub fn read_datetime(&self) -> Option<DateTime> {
        // Wait until UIP flag is clear to prevent torn reads
        for _ in 0..10_000 {
            if !self.is_update_in_progress() {
                break;
            }
            core::hint::spin_loop();
        }

        let mut sec = self.read_cmos_register(RTC_REG_SECONDS);
        let mut min = self.read_cmos_register(RTC_REG_MINUTES);
        let mut hour = self.read_cmos_register(RTC_REG_HOURS);
        let mut day = self.read_cmos_register(RTC_REG_DAY);
        let mut month = self.read_cmos_register(RTC_REG_MONTH);
        let mut year = self.read_cmos_register(RTC_REG_YEAR);
        let status_b = self.read_cmos_register(RTC_REG_STATUS_B);

        // Decode BCD format if bit 2 of Status Register B is 0
        let is_bcd = (status_b & 0x04) == 0;
        if is_bcd {
            sec = ((sec >> 4) * 10) + (sec & 0x0F);
            min = ((min >> 4) * 10) + (min & 0x0F);
            hour = (((hour & 0x70) >> 4) * 10) + (hour & 0x0F);
            day = ((day >> 4) * 10) + (day & 0x0F);
            month = ((month >> 4) * 10) + (month & 0x0F);
            year = ((year >> 4) * 10) + (year & 0x0F);
        }

        // Handle 12-hour clock mode if bit 1 of Status Register B is 0
        let is_24_hour = (status_b & 0x02) != 0;
        if !is_24_hour && (hour & 0x80) != 0 {
            hour = ((hour & 0x7F) + 12) % 24;
        }

        // Assuming 2000s century for two-digit CMOS year
        let full_year = 2000u16 + (year as u16);

        if month < 1 || month > 12 || day < 1 || day > 31 || hour > 23 || min > 59 || sec > 59 {
            return None;
        }

        let unix_timestamp = datetime_to_unix_epoch(full_year, month, day, hour, min, sec);

        Some(DateTime {
            year: full_year,
            month,
            day,
            hours: hour,
            minutes: min,
            seconds: sec,
            unix_timestamp,
        })
    }

    /// Returns the provisional wall-clock time in UTC seconds since Unix epoch.
    pub fn get_provisional_wall_clock(&self) -> Result<u64, ()> {
        if !self.is_provisional_valid {
            return Err(());
        }
        // Advance provisional wall-clock by elapsed monotonic time since boot
        let current_rdtsc = unsafe { rdtsc() };
        let elapsed_ticks = current_rdtsc.saturating_sub(self.initial_rdtsc);
        // Approximate 2.0 GHz CPU frequency for rdtsc tick conversion (1 sec = 2_000_000_000 ticks)
        let elapsed_secs = elapsed_ticks / 2_000_000_000;
        Ok(self.initial_boot_epoch + elapsed_secs)
    }

    /// Returns strictly monotonic time in nanoseconds since boot.
    pub fn get_monotonic_nanos(&self) -> u64 {
        let current_rdtsc = unsafe { rdtsc() };
        let elapsed_ticks = current_rdtsc.saturating_sub(self.initial_rdtsc);
        // Approximate 2 GHz: 1 tick = 0.5 nanoseconds
        elapsed_ticks / 2
    }

    #[inline]
    fn is_update_in_progress(&self) -> bool {
        (self.read_cmos_register(RTC_REG_STATUS_A) & 0x80) != 0
    }

    #[inline]
    fn read_cmos_register(&self, reg: u8) -> u8 {
        unsafe {
            outb(CMOS_ADDR_PORT, reg);
            inb(CMOS_DATA_PORT)
        }
    }
}

/// Converts Gregorian Date & Time into Unix Epoch seconds.
pub fn datetime_to_unix_epoch(year: u16, month: u8, day: u8, hour: u8, min: u8, sec: u8) -> u64 {
    let mut days: u64 = 0;

    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }

    let days_in_months: [u64; 12] = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];

    for m in 1..month {
        days += days_in_months[(m - 1) as usize];
    }

    days += (day.saturating_sub(1)) as u64;

    let hours_total = days * 24 + (hour as u64);
    let mins_total = hours_total * 60 + (min as u64);
    mins_total * 60 + (sec as u64)
}

#[inline]
fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
