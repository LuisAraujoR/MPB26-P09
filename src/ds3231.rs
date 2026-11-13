//! Driver para el RTC DS3231 / DS3231M vía I2C
#![allow(dead_code)]

use esp_idf_svc::hal::i2c::I2cDriver;
use esp_idf_svc::hal::sys::EspError;

pub const DS3231_I2C_ADDR: u8 = 0x68;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: u16,   // Ej: 2026
    pub month: u8,   // 1..=12
    pub day: u8,     // 1..=31
    pub hour: u8,    // 0..=23
    pub minute: u8,  // 0..=59
    pub second: u8,  // 0..=59
    pub day_of_week: u8, // 1..=7
}

impl DateTime {
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8, day_of_week: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            day_of_week,
        }
    }
}

pub struct Ds3231<'a> {
    i2c: I2cDriver<'a>,
}

impl<'a> Ds3231<'a> {
    pub fn new(i2c: I2cDriver<'a>) -> Self {
        Self { i2c }
    }

    /// Lee la fecha y hora actual del DS3231
    pub fn get_datetime(&mut self) -> Result<DateTime, EspError> {
        let mut buf = [0u8; 7];
        // Enviar dirección de registro 0x00 y leer 7 registros consecutivos (con repeated start)
        self.i2c.write_read(DS3231_I2C_ADDR, &[0x00], &mut buf, 1000)?;

        let second = bcd_to_dec(buf[0] & 0x7F);
        let minute = bcd_to_dec(buf[1] & 0x7F);
        
        // Formato 24h: si el bit 6 es 0, es formato 24h
        let hour = if (buf[2] & 0x40) != 0 {
            // Formato 12 horas
            let h = bcd_to_dec(buf[2] & 0x1F);
            let pm = (buf[2] & 0x20) != 0;
            if pm && h < 12 { h + 12 } else if !pm && h == 12 { 0 } else { h }
        } else {
            // Formato 24 horas
            bcd_to_dec(buf[2] & 0x3F)
        };

        let day_of_week = buf[3] & 0x07;
        let day = bcd_to_dec(buf[4] & 0x3F);
        let century = (buf[5] & 0x80) != 0;
        let month = bcd_to_dec(buf[5] & 0x1F);
        let year_raw = bcd_to_dec(buf[6]);
        let year = if century { 2100 + year_raw as u16 } else { 2000 + year_raw as u16 };

        Ok(DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            day_of_week,
        })
    }

    /// Configura una nueva fecha y hora en el DS3231
    pub fn set_datetime(&mut self, dt: &DateTime) -> Result<(), EspError> {
        let year_byte = (dt.year % 100) as u8;
        let century_bit = if dt.year >= 2100 { 0x80 } else { 0x00 };

        let buf = [
            0x00, // Registro base
            dec_to_bcd(dt.second),
            dec_to_bcd(dt.minute),
            dec_to_bcd(dt.hour) & 0x3F, // Modo 24h
            dt.day_of_week & 0x07,
            dec_to_bcd(dt.day) & 0x3F,
            (dec_to_bcd(dt.month) & 0x1F) | century_bit,
            dec_to_bcd(year_byte),
        ];

        self.i2c.write(DS3231_I2C_ADDR, &buf, 1000)
    }

    /// Lee la temperatura del sensor interno del DS3231 en grados Celsius
    pub fn get_temperature(&mut self) -> Result<f32, EspError> {
        let mut buf = [0u8; 2];
        self.i2c.write_read(DS3231_I2C_ADDR, &[0x11], &mut buf, 1000)?;

        let msb = buf[0] as i8;
        let lsb = (buf[1] >> 6) as f32 * 0.25;
        Ok(msb as f32 + lsb)
    }
}

#[inline]
fn bcd_to_dec(val: u8) -> u8 {
    ((val >> 4) * 10) + (val & 0x0F)
}

#[inline]
fn dec_to_bcd(val: u8) -> u8 {
    ((val / 10) << 4) | (val % 10)
}

