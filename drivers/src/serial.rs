use core::fmt;
use core::fmt::Write;
use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::{Config, Uart16550Tty};

#[cfg(target_arch = "x86_64")]
type SerialPort = Uart16550Tty<uart_16550::backend::PioBackend>;
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
type SerialPort = Uart16550Tty<uart_16550::backend::MmioBackend>;

lazy_static! {
    pub static ref SERIAL_WRITER: Mutex<SerialPort> = {
        #[cfg(target_arch = "x86_64")]
        let serial_port = unsafe {
            Uart16550Tty::new_port(0x3F8, Config::default()).expect("failed to init serial")
        };

        #[cfg(target_arch = "riscv64")]
        let serial_port = unsafe {
            Uart16550Tty::new_mmio(
                core::ptr::NonNull::new(0x10000000 as *mut u8)
                    .expect("serial: null UART MMIO address on riscv64"),
                1,
                Config::default(),
            )
            .expect("serial: failed to init riscv64 UART")
        };

        #[cfg(target_arch = "aarch64")]
        let serial_port = unsafe {
            Uart16550Tty::new_mmio(
                core::ptr::NonNull::new(0x09000000 as *mut u8)
                    .expect("serial: null UART MMIO address on aarch64"),
                1,
                Config::default(),
            )
            .expect("serial: failed to init aarch64 UART")
        };

        #[cfg(target_arch = "loongarch64")]
        let serial_port = unsafe {
            Uart16550Tty::new_mmio(
                core::ptr::NonNull::new(0x1fe001e0 as *mut u8)
                    .expect("serial: null UART MMIO address on loongarch64"),
                1,
                Config::default(),
            )
            .expect("serial: failed to init loongarch64 UART")
        };

        Mutex::new(serial_port)
    };
}

// Implement the `fmt::Write` trait for our global SERIAL_WRITER.
// This allows us to use it with Rust's formatting macros like `println!` and `write!`.
#[doc(hidden)]
pub fn print(args: fmt::Arguments) {
    let mut writer = SERIAL_WRITER.lock();
    writer.write_fmt(args).expect("Printing to serial failed");
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

/// Initializes the logger, allowing the `log` crate macros to work.
/// # Panics
/// when initialization fails
pub fn init() {
    // Set our serial logger as the global logger.
    // We set the max level to Info, meaning Trace and Debug messages will be ignored.
    log::set_logger(&SERIAL_LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .expect("Failed to set logger");
}

// A simple logger that writes to the serial port.
static SERIAL_LOGGER: SerialLogger = SerialLogger;

struct SerialLogger;

impl log::Log for SerialLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // We can filter messages here if we want.
        // For now, let's enable all messages up to our max level.
        metadata.level() <= log::LevelFilter::Info
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            serial_println!("[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
