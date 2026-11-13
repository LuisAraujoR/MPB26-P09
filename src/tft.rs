//! Driver para pantalla TFT SPI 1.8" 128x160 (Controlador ST7735 / ST7735S)
#![allow(dead_code)]

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{Output, PinDriver};
use esp_idf_svc::hal::spi::{SpiDeviceDriver, SpiDriver};
use esp_idf_svc::hal::sys::EspError;

use crate::font::{get_char_bitmap, FONT_HEIGHT, FONT_WIDTH};

// Comandos ST7735
const CMD_SWRESET: u8 = 0x01;
const CMD_SLPOUT: u8 = 0x11;
const CMD_NORON: u8 = 0x13;
const CMD_INVOFF: u8 = 0x20;
const CMD_INVON: u8 = 0x21;
const CMD_DISPOFF: u8 = 0x28;
const CMD_DISPON: u8 = 0x29;
const CMD_CASET: u8 = 0x2A;
const CMD_RASET: u8 = 0x2B;
const CMD_RAMWR: u8 = 0x2C;
const CMD_MADCTL: u8 = 0x36;
const CMD_COLMOD: u8 = 0x3A;
const CMD_FRMCTR1: u8 = 0xB1;
const CMD_FRMCTR2: u8 = 0xB2;
const CMD_FRMCTR3: u8 = 0xB3;
const CMD_INVCTR: u8 = 0xB4;
const CMD_PWCTR1: u8 = 0xC0;
const CMD_PWCTR2: u8 = 0xC1;
const CMD_PWCTR3: u8 = 0xC2;
const CMD_PWCTR4: u8 = 0xC3;
const CMD_PWCTR5: u8 = 0xC4;
const CMD_VMCTR1: u8 = 0xC5;
const CMD_GMCTRP1: u8 = 0xE0;
const CMD_GMCTRN1: u8 = 0xE1;

// Colores comunes en formato RGB565 (16 bits)
pub mod color {
    pub const BLACK: u16 = 0x0000;
    pub const WHITE: u16 = 0xFFFF;
    pub const RED: u16 = 0xF800;
    // 1111 1000 0000 0000
    pub const GREEN: u16 = 0x07E0;
    // 0000 0111 1110 0000
    pub const BLUE: u16 = 0x001F;
    // 0000 0000 0001 1111
    pub const YELLOW: u16 = 0xFFE0;

    pub const CYAN: u16 = 0x07FF;
    pub const MAGENTA: u16 = 0xF81F;
    pub const ORANGE: u16 = 0xFD20;
    pub const DARK_GREY: u16 = 0x39E7;
    pub const LIGHT_GREY: u16 = 0xCE79;

    pub const NAVY: u16 = 0x000F;

    #[inline]
    pub const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
        ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | ((b as u16) >> 3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,          // 128x160
    Landscape,         // 160x128
    PortraitInverted,  // 128x160 invertido
    LandscapeInverted, // 160x128 invertido
}

pub struct Tft18<'a> {
    spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
    dc: PinDriver<'a, Output>,
    rst: PinDriver<'a, Output>,
    cs: PinDriver<'a, Output>,
    orientation: Orientation,
    width: u16,
    height: u16,
    col_offset: u16,
    row_offset: u16,
}

impl<'a> Tft18<'a> {
    pub fn new(
        spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
        dc: PinDriver<'a, Output>,
        rst: PinDriver<'a, Output>,
        cs: PinDriver<'a, Output>,
    ) -> Self {
        Self {
            spi,
            dc,
            rst,
            cs,
            orientation: Orientation::Landscape,
            width: 160,
            height: 128,
            col_offset: 0,
            row_offset: 0,
        }
    }

    /// Inicializa la pantalla ST7735 con la secuencia de arranque
    pub fn init(&mut self) -> Result<(), EspError> {
        // Reset por hardware
        self.cs.set_high()?;
        self.rst.set_high()?;
        Ets::delay_ms(10);
        self.rst.set_low()?;
        Ets::delay_ms(20);
        self.rst.set_high()?;
        Ets::delay_ms(150);

        // Reset por software
        self.send_command(CMD_SWRESET)?;
        Ets::delay_ms(150);

        // Salir de modo suspensión
        self.send_command(CMD_SLPOUT)?;
        Ets::delay_ms(200);

        // Control de frecuencia de refresco (Frame Rate)
        self.send_command_with_data(CMD_FRMCTR1, &[0x01, 0x2C, 0x2D])?;
        self.send_command_with_data(CMD_FRMCTR2, &[0x01, 0x2C, 0x2D])?;
        self.send_command_with_data(CMD_FRMCTR3, &[0x01, 0x2C, 0x2D, 0x01, 0x2C, 0x2D])?;

        // Control de inversión de display (Line Inversion)
        self.send_command_with_data(CMD_INVCTR, &[0x07])?;

        // Control de energía
        self.send_command_with_data(CMD_PWCTR1, &[0xA2, 0x02, 0x84])?;
        self.send_command_with_data(CMD_PWCTR2, &[0xC5])?;
        self.send_command_with_data(CMD_PWCTR3, &[0x0A, 0x00])?;
        self.send_command_with_data(CMD_PWCTR4, &[0x8A, 0x2A])?;
        self.send_command_with_data(CMD_PWCTR5, &[0x8A, 0xEE])?;

        // VCOM
        self.send_command_with_data(CMD_VMCTR1, &[0x0E])?;

        // Inversión desactivada
        self.send_command(CMD_INVOFF)?;

        // Formato de pixel de color: 16-bit/pixel (RGB 5-6-5)
        self.send_command_with_data(CMD_COLMOD, &[0x05])?;

        // Curvas Gamma
        self.send_command_with_data(
            CMD_GMCTRP1,
            &[
                0x02, 0x1C, 0x07, 0x12, 0x37, 0x32, 0x29, 0x2D, 0x29, 0x25, 0x2B, 0x39, 0x00, 0x01,
                0x03, 0x10,
            ],
        )?;
        self.send_command_with_data(
            CMD_GMCTRN1,
            &[
                0x03, 0x1D, 0x07, 0x06, 0x2E, 0x2C, 0x29, 0x2D, 0x2E, 0x2E, 0x37, 0x3F, 0x00, 0x00,
                0x02, 0x10,
            ],
        )?;

        // Normal display ON
        self.send_command(CMD_NORON)?;
        Ets::delay_ms(10);

        // Encender pantalla
        self.send_command(CMD_DISPON)?;
        Ets::delay_ms(100);

        // Aplicar orientación por defecto
        self.set_orientation(self.orientation)?;

        // Limpiar pantalla en negro
        self.clear(color::BLACK)?;

        Ok(())
    }

    /// Configura la orientación de la pantalla y actualiza ancho y alto
    pub fn set_orientation(&mut self, orientation: Orientation) -> Result<(), EspError> {
        self.orientation = orientation;
        // Bit 3 = 1 para modo BGR habitual en módulos 1.8 TFT rojos/azules
        let (madctl_val, w, h, col_off, row_off) = match orientation {
            Orientation::Portrait => (0xC8, 128, 160, 0, 0),
            Orientation::Landscape => (0xA8, 160, 128, 0, 0),
            Orientation::PortraitInverted => (0x08, 128, 160, 0, 0),
            Orientation::LandscapeInverted => (0x68, 160, 128, 0, 0),
        };

        self.width = w;
        self.height = h;
        self.col_offset = col_off;
        self.row_offset = row_off;

        self.send_command_with_data(CMD_MADCTL, &[madctl_val])
    }

    /// Configura el desplazamiento de columnas y filas (offset) si el módulo físico lo requiere
    pub fn set_offsets(&mut self, col_offset: u16, row_offset: u16) {
        self.col_offset = col_offset;
        self.row_offset = row_offset;
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// Define la ventana activa de memoria de video para escritura
    fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) -> Result<(), EspError> {
        let x0 = x0 + self.col_offset;
        let x1 = x1 + self.col_offset;
        let y0 = y0 + self.row_offset;
        let y1 = y1 + self.row_offset;

        let col_data = [
            (x0 >> 8) as u8,
            (x0 & 0xFF) as u8,
            (x1 >> 8) as u8,
            (x1 & 0xFF) as u8,
        ];
        let row_data = [
            (y0 >> 8) as u8,
            (y0 & 0xFF) as u8,
            (y1 >> 8) as u8,
            (y1 & 0xFF) as u8,
        ];

        self.send_command_with_data(CMD_CASET, &col_data)?;
        self.send_command_with_data(CMD_RASET, &row_data)?;
        self.send_command(CMD_RAMWR)?;
        Ok(())
    }

    /// Limpia toda la pantalla con un color de fondo
    pub fn clear(&mut self, color: u16) -> Result<(), EspError> {
        self.fill_rect(0, 0, self.width, self.height, color)
    }

    /// Dibuja un pixel individual
    pub fn draw_pixel(&mut self, x: u16, y: u16, color: u16) -> Result<(), EspError> {
        if x >= self.width || y >= self.height {
            return Ok(());
        }
        self.set_address_window(x, y, x, y)?;
        let color_bytes = [(color >> 8) as u8, (color & 0xFF) as u8];
        self.send_data(&color_bytes)
    }

    /// Rellena un rectángulo de ancho `w` y alto `h` a partir de `(x, y)`
    pub fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        color: u16,
    ) -> Result<(), EspError> {
        if x >= self.width || y >= self.height || w == 0 || h == 0 {
            return Ok(());
        }
        let x1 = (x + w - 1).min(self.width - 1);
        let y1 = (y + h - 1).min(self.height - 1);

        self.set_address_window(x, y, x1, y1)?;

        let count = ((x1 - x + 1) as usize) * ((y1 - y + 1) as usize);
        let hi = (color >> 8) as u8;
        let lo = (color & 0xFF) as u8;

        // Buffer de escritura por bloques para máxima velocidad SPI
        let mut buffer = [0u8; 128];
        for i in (0..buffer.len()).step_by(2) {
            buffer[i] = hi;
            buffer[i + 1] = lo;
        }

        let mut remaining = count;
        let chunk_pixels = buffer.len() / 2;

        self.cs.set_low()?;
        self.dc.set_high()?;
        while remaining > 0 {
            let to_send_pixels = remaining.min(chunk_pixels);
            self.spi.write(&buffer[..to_send_pixels * 2])?;
            remaining -= to_send_pixels;
        }
        self.cs.set_high()?;

        Ok(())
    }

    /// Dibuja el contorno de un rectángulo
    pub fn draw_rect(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        color: u16,
    ) -> Result<(), EspError> {
        if w == 0 || h == 0 {
            return Ok(());
        }
        self.fill_rect(x, y, w, 1, color)?; // Superior
        self.fill_rect(x, y + h - 1, w, 1, color)?; // Inferior
        self.fill_rect(x, y, 1, h, color)?; // Izquierda
        self.fill_rect(x + w - 1, y, 1, h, color)?; // Derecha
        Ok(())
    }

    /// Dibuja un carácter con factor de escala `scale` (1 = 5x7 px, 2 = 10x14 px, etc.)
    pub fn draw_char(
        &mut self,
        x: u16,
        y: u16,
        c: char,
        fg: u16,
        bg: u16,
        scale: u8,
    ) -> Result<(), EspError> {
        let bitmap = match get_char_bitmap(c) {
            Some(b) => *b,
            None => [0u8; FONT_WIDTH],
        };

        let scale = scale.max(1) as u16;

        for col in 0..FONT_WIDTH {
            let line = bitmap[col];
            for row in 0..FONT_HEIGHT {
                let bit = (line >> row) & 0x01;
                let color = if bit != 0 { fg } else { bg };
                let px = x + (col as u16) * scale;
                let py = y + (row as u16) * scale;

                if scale == 1 {
                    self.draw_pixel(px, py, color)?;
                } else {
                    self.fill_rect(px, py, scale, scale, color)?;
                }
            }
        }

        // Columna de separación entre caracteres con color de fondo
        let px = x + (FONT_WIDTH as u16) * scale;
        if scale == 1 {
            for row in 0..FONT_HEIGHT {
                self.draw_pixel(px, y + row as u16, bg)?;
            }
        } else {
            self.fill_rect(px, y, scale, (FONT_HEIGHT as u16) * scale, bg)?;
        }

        Ok(())
    }

    /// Escribe una cadena de texto en las coordenadas `(x, y)` con escala
    pub fn draw_text(
        &mut self,
        mut x: u16,
        y: u16,
        text: &str,
        fg: u16,
        bg: u16,
        scale: u8,
    ) -> Result<(), EspError> {
        let scale = scale.max(1);
        let char_spacing = (FONT_WIDTH as u16 + 1) * (scale as u16);

        for c in text.chars() {
            if x + char_spacing > self.width {
                break;
            }
            self.draw_char(x, y, c, fg, bg, scale)?;
            x += char_spacing;
        }

        Ok(())
    }

    // --- Funciones internas de bajo nivel SPI ---

    fn send_command(&mut self, cmd: u8) -> Result<(), EspError> {
        self.dc.set_low()?;
        self.cs.set_low()?;
        let res = self.spi.write(&[cmd]);
        self.cs.set_high()?;
        res
    }

    fn send_command_with_data(&mut self, cmd: u8, data: &[u8]) -> Result<(), EspError> {
        self.send_command(cmd)?;
        self.send_data(data)
    }

    fn send_data(&mut self, data: &[u8]) -> Result<(), EspError> {
        if data.is_empty() {
            return Ok(());
        }
        self.dc.set_high()?;
        self.cs.set_low()?;
        let res = self.spi.write(data);
        self.cs.set_high()?;
        res
    }
}
