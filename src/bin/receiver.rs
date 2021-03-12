#![no_std]
#![no_main]

use panic_rtt_target as _;
use rtt_target::rprintln;
use rtt_target::rtt_init_print;

use cortex_m::asm::delay;
use cortex_m_rt::entry;
use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::spi::Spi;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    // setup stm32 peripherals
    let dp = pac::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);

    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(48.mhz())
        .pclk1(24.mhz())
        .freeze(&mut flash.acr);

    // Configure the on-board LED (PC13, green)
    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    led.set_high().ok(); // Turn off

    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    // Support beeper on pb9
    let mut beep = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);

    // Two dedicated pins and one SPI port for the nrf24l01
    let ce = gpiob.pb0.into_push_pull_output(&mut gpiob.crl);
    let mut csn = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
    csn.set_high().ok();
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6;
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let spi = Spi::spi1(
        dp.SPI1,
        (sck, miso, mosi),
        &mut afio.mapr,
        calliope_nrf24::spi_mode(),
        2.mhz(),
        clocks,
        &mut rcc.apb2,
    );

    // initialize receiver

    let group = 7;
    let radio: calliope_nrf24::Standby<_>;
    match calliope_nrf24::Standby::new(ce, csn, spi, group) {
        Ok(r) => {
            radio = r;
        }
        Err(e) => {
            // can't do much, here -> reset
            rprintln!("Err: {:?}", e);
            delay(clocks.sysclk().0);
            cortex_m::peripheral::SCB::sys_reset();
        }
    }

    let mut rx = radio.rx().unwrap();

    rprintln!("now running");
    loop {
        let mut buf = [0u8; 32];
        match rx.receive(&mut buf) {
            Ok(Some(payload)) => {
                // turn LED on
                led.set_low().ok();
                // print received payload and play a short beep
                // (more likely a clicking sound)
                beep.set_high().ok();
                rprintln!("Payload: {:02x?}", payload);
                beep.set_low().ok();
            }
            Err(e) => rprintln!("rx.read: {:?}", e),
            _ => {
                // all fine, just no data
                // turn LED off
                led.set_high().ok();
            }
        }
    }
}
