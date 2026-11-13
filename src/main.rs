mod ds3231;
mod font;
mod tft;

use std::thread::sleep;
use std::time::Duration;

use esp_idf_svc::hal::gpio::{AnyIOPin, AnyOutputPin, PinDriver};
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::spi::config::{Config, DriverConfig};
use esp_idf_svc::hal::spi::{SpiDeviceDriver, SpiDriver};
use esp_idf_svc::hal::units::FromValueType;
use log::{error, info};

use ds3231::{DateTime, Ds3231};
use tft::{color, Orientation, Tft18};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Patches necesarios para esp-idf
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Iniciando sistema ESP32-S3: TFT SPI 1.8\" + RTC DS3231M...");

    let peripherals = Peripherals::take()?;

    // ==========================================
    // 1. Configuración de pantalla TFT SPI 1.8"
    // ==========================================
    // SCK:   GPIO3
    // SDA:   GPIO8 (MOSI)
    // CS:    GPIO16
    // RESET: GPIO17
    // A0/DC: GPIO18
    let spi_driver = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio3,   // SCK
        peripherals.pins.gpio8,   // SDA / MOSI
        Option::<AnyIOPin>::None, // MISO no usado
        &DriverConfig::default(),
    )?;

    let spi_device = SpiDeviceDriver::new(
        spi_driver,
        Option::<AnyOutputPin>::None, // CS controlado manualmente vía PinDriver
        &Config::default().baudrate(26.MHz().into()),
    )?;

    let dc = PinDriver::output(peripherals.pins.gpio18.degrade_output())?;
    let rst = PinDriver::output(peripherals.pins.gpio17.degrade_output())?;
    let cs = PinDriver::output(peripherals.pins.gpio16.degrade_output())?;

    let mut display = Tft18::new(spi_device, dc, rst, cs);

    info!("Inicializando pantalla TFT ST7735...");
    display.init()?;
    display.set_orientation(Orientation::Landscape)?;
    display.clear(color::BLACK)?;

    // ==========================================
    // 2. Configuración de RTC DS3231M (I2C)
    // ==========================================
    // SCL: GPIO41
    // SDA: GPIO40--
    let i2c_config = I2cConfig::new().baudrate(100.kHz().into());
    let i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio40, // SDA
        peripherals.pins.gpio41, // SCL
        &i2c_config,
    )?;

    let mut rtc = Ds3231::new(i2c);
    info!("RTC DS3231 inicializado en pines SCL=GPIO40, SDA=GPIO41.");

    // Dibujar interfaz gráfica inicial estática
    draw_ui_template(&mut display)?;

    let mut last_second: u8 = 255;

    // Ajustar fecha y hora inicial en el RTC: Año, Mes, Día, Hora, Minuto, Segundo, DíaDeLaSemana (1=Dom..7=Sab)
    let datetime = DateTime::new(2026, 11, 12, 17, 1, 9, 5);
    rtc.set_datetime(&datetime)?;

    loop {
        match rtc.get_datetime() {
            Ok(dt) => {
                // Solo refrescar la pantalla cuando cambie el segundo para optimizar
                if dt.second != last_second {
                    last_second = dt.second;

                    // Formatear Hora: HH:MM:SS
                    let time_str = format!("{:02}:{:02}:{:02}", dt.hour, dt.minute, dt.second);
                    // Formatear Fecha: YYYY-MM-DD
                    let date_str = format!("{:02}-{:02}-{:04}", dt.day, dt.month, dt.year);

                    // Formatear Día de la semana
                    let day_name = match dt.day_of_week {
                        1 => "DOMINGO",
                        2 => "LUNES",
                        3 => "MARTES",
                        4 => "MIERCOLES",
                        5 => "JUEVES",
                        6 => "VIERNES",
                        7 => "SABADO",
                        _ => "---",
                    };

                    // Renderizar Hora grande (Escala 2: cada carácter mide 12x14 px, 8 chars = 96 px)
                    // Centrado horizontal en 160 px: (160 - 96) / 2 = 32
                    display.draw_text(32, 48, &time_str, color::YELLOW, color::BLACK, 2)?;

                    // Renderizar Fecha y Día (Escala 1: 6x8 px por char)
                    display.draw_text(
                        20,
                        84,
                        &format!("FECHA: {}", date_str),
                        color::CYAN,
                        color::BLACK,
                        1,
                    )?;
                    display.draw_text(
                        20,
                        98,
                        &format!("DIA:   {}", day_name),
                        color::WHITE,
                        color::BLACK,
                        1,
                    )?;

                    // Leer y mostrar temperatura interna del DS3231M
                    if let Ok(temp) = rtc.get_temperature() {
                        let temp_str = format!("TEMP:  {:.1} C", temp);
                        display.draw_text(20, 112, &temp_str, color::GREEN, color::BLACK, 1)?;
                    }

                    info!("Hora RTC: {} | Fecha: {}", time_str, date_str);
                }
            }
            Err(e) => {
                error!("Error al leer del RTC DS3231: {:?}", e);
                display.draw_text(10, 50, "ERROR RTC I2C", color::RED, color::BLACK, 2)?;
            }
        }

        sleep(Duration::from_millis(200));
    }
}

/// Dibuja el marco y los elementos estáticos de la pantalla
fn draw_ui_template(display: &mut Tft18) -> Result<(), esp_idf_svc::hal::sys::EspError> {
    // Fondo negro y marco rectangular exterior
    display.clear(color::BLACK)?;
    display.draw_rect(2, 2, 156, 124, color::NAVY)?;
    display.draw_rect(3, 3, 154, 122, color::BLUE)?;

    // Encabezado con banner superior
    display.fill_rect(4, 4, 152, 20, color::NAVY)?;
    display.draw_text(50, 10, "RTC DS3231", color::WHITE, color::NAVY, 1)?;

    // Línea divisoria
    display.fill_rect(10, 36, 140, 2, color::DARK_GREY)?;
    display.fill_rect(10, 76, 140, 2, color::DARK_GREY)?;

    Ok(())
}
