use core::fmt;
use core::fmt::Write;
use lazy_static::lazy_static;
use spin::Mutex;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use uart_16550::SerialPort;
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
use uart_16550::MmioSerialPort as SerialPort;
lazy_static! {
    pub static ref SERIAL_WRITER: Mutex<SerialPort> = {
        // We use conditional compilation to select the correct initialization
        // method and address based on the target architecture.
        let mut serial_port = {
            #[cfg(target_arch = "x86_64")]
            {
                // For x86_64, we use I/O Ports. The standard address for COM1 is 0x3F8.
                // The `uart_16550` crate's `x86_64` feature provides the `new()` constructor.
                unsafe { SerialPort::new(0x3F8) }
            }

            #[cfg(target_arch = "riscv64")]
            {
                // For RISC-V, we use Memory-Mapped I/O.
                // The standard address for the UART in QEMU's 'virt' machine is 0x10000000.
                // We must use the `new_mmio()` constructor for this.
                unsafe { SerialPort::new(0x10000000) }
            }
        };

        serial_port.init();
        Mutex::new(serial_port)
    };
}

// Implement the `fmt::Write` trait for our global SERIAL_WRITER.
// This allows us to use it with Rust's formatting macros like `println!` and `write!`.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    // Add a guard to prevent deadlocks if a panic occurs while the lock is held.
    if let Some(mut writer) = SERIAL_WRITER.try_lock() {
        writer.write_fmt(args).expect("Printing to serial failed");
    }
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
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
