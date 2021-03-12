#![no_std]

use rtt_target::rprintln;

use embedded_hal::digital::v2::OutputPin;
use embedded_nrf24l01::Device;
use embedded_nrf24l01::RxMode;
use embedded_nrf24l01::StandbyMode;
use embedded_nrf24l01::TxMode;
use embedded_nrf24l01::NRF24L01;
use embedded_nrf24l01::{Configuration, CrcMode, DataRate};

use core::hash::Hasher;

pub use embedded_nrf24l01::setup::spi_mode;

const WHITEIV: u8 = 0x18; // as defined in micro:bit code

use core::fmt::Debug;
use embedded_hal::blocking::spi::Transfer as SpiTransfer;

pub struct Standby<D: Device>(StandbyMode<D>, u8);

pub struct Rx<D: Device>(RxMode<D>, u8);

pub struct Tx<D: Device>(TxMode<D>, u8);

impl<D: Device> Rx<D> {
    pub fn receive<'a>(&mut self, buf: &'a mut [u8; 32]) -> Result<Option<&'a [u8]>, D::Error> {
        let rx = &mut self.0;
        let group = self.1;
        match rx.can_read() {
            Ok(Some(_)) => {
                match rx.read() {
                    Ok(payload) => {
                        let count = payload.len().min(32);

                        let mut crc = crc16::State::<crc16::CCITT_FALSE>::new();

                        crc.write_u8(b't'.reverse_bits());
                        crc.write_u8(b'i'.reverse_bits());
                        crc.write_u8(b'b'.reverse_bits());
                        crc.write_u8(b'u'.reverse_bits());
                        crc.write_u8(group.reverse_bits());

                        let mut data = [0u8; 32];
                        let mut whiten = WHITEIV | 0x40;
                        let mut len = 0usize;
                        whiten = whiten.reverse_bits() >> 1; // reverse 7 bits
                        for i in 0..count {
                            data[i] = payload[i];
                            for b in 0..8 {
                                let m = 1 << (7 - b);
                                whiten <<= 1;
                                if whiten & 0x80 != 0 {
                                    whiten ^= 0x11;
                                    data[i] ^= m;
                                }
                            }
                            if i <= len {
                                crc.write_u8(data[i]);
                                data[i] = data[i].reverse_bits();
                            }
                            if i == 0 {
                                len = data[i] as usize;
                            }
                        }
                        let crc_calc = crc.finish();

                        rprintln!(
                            "Payload: {:02x?} / Crc: {:08x}",
                            &data[0..count.min(len + 3)],
                            crc_calc
                        );
                        if count >= (len + 3) as _
                            && data[(len as usize + 1)..=(len as usize + 2) as usize]
                                == [(crc_calc >> 8) as u8, crc_calc as u8]
                        {
                            rprintln!("crc ok");
                            if len >= 3 {
                                buf[0..(len - 3)].copy_from_slice(&data[4..(len + 1)]);
                                Ok(Some(&buf[0..count.min(len - 3)]))
                            } else {
                                Ok(None)
                            }
                        } else {
                            rprintln!("crc bad");
                            // TODO: error code?
                            Ok(None)
                        }
                    }
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e),
            _ => Ok(None),
        }
    }
}

impl<D: Device> Tx<D> {
    pub fn transmit(&mut self, payload: &[u8]) -> Result<bool, D::Error> {
        let tx = &mut self.0;
        let group = &self.1;
        if let Ok(true) = tx.can_send() {
            let mut data = [0u8; 32];
            let len = (payload.len() + 3).min(data.len() - 3); // TODO silently truncates long messages
            data[0] = len as u8;
            data[1] = 1; // TODO as seen from Calliope; check meaning and make configurable if appropriate
            data[2] = 0;
            data[3] = 1;
            //let mut data = [0u8, 1, 0, 1, b't', b'e', b's', b't', 0, 0];
            data[4..(len + 1)].copy_from_slice(&payload[0..(len - 3)]);

            let mut crc = crc16::State::<crc16::CCITT_FALSE>::new();
            crc.write_u8(b't'.reverse_bits());
            crc.write_u8(b'i'.reverse_bits());
            crc.write_u8(b'b'.reverse_bits());
            crc.write_u8(b'u'.reverse_bits());
            crc.write_u8(group.reverse_bits());
            data[0..=len].iter_mut().for_each(|b| {
                *b = b.reverse_bits();
                crc.write_u8(*b);
            });
            let digest = crc.finish();
            data[len + 1] = ((digest >> 8) & 0xff) as u8;
            data[len + 2] = (digest & 0xff) as u8;
            // whiten
            let mut whiten = WHITEIV | 0x40;
            whiten = whiten.reverse_bits() >> 1; // reverse 7 bits
            for d in &mut data {
                for b in 0..8 {
                    let m = 1 << (7 - b);
                    whiten <<= 1;
                    if whiten & 0x80 != 0 {
                        whiten ^= 0x11;
                        *d ^= m;
                    }
                }
            }

            tx.send(&data)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl<
        E: Debug,
        CE: OutputPin<Error = E>,
        CSN: OutputPin<Error = E>,
        SPI: SpiTransfer<u8, Error = SPIE>,
        SPIE: Debug,
    > Standby<NRF24L01<E, CE, CSN, SPI>>
{
    pub fn rx(
        self,
    ) -> Result<
        Rx<NRF24L01<E, CE, CSN, SPI>>,
        (NRF24L01<E, CE, CSN, SPI>, embedded_nrf24l01::Error<SPIE>),
    > {
        let group = self.1;
        self.0.rx().map(|rx| Rx(rx, group))
    }

    pub fn tx(
        self,
    ) -> Result<
        Tx<NRF24L01<E, CE, CSN, SPI>>,
        (NRF24L01<E, CE, CSN, SPI>, embedded_nrf24l01::Error<SPIE>),
    > {
        let group = self.1;
        self.0.tx().map(|tx| Tx(tx, group))
    }

    pub fn new(ce: CE, csn: CSN, spi: SPI, group: u8) -> Result<Self, embedded_nrf24l01::Error<SPIE>> {
        match NRF24L01::new(ce, csn, spi) {
            Ok(nrf24) => {
                let mut nrf24: StandbyMode<NRF24L01<E, CE, CSN, SPI>> = nrf24;
                nrf24.set_frequency(7).unwrap(); // Error handling could be improved
                nrf24.set_auto_retransmit(0, 0).unwrap();
                nrf24.set_rf(&DataRate::R1Mbps, 3).unwrap();
                nrf24
                    .set_pipes_rx_enable(&[true, false, false, false, false, false])
                    .unwrap();
                nrf24.set_auto_ack(&[false; 6]).unwrap();
                nrf24.set_crc(CrcMode::Disabled).unwrap();
                //not yet available in published version of embedded-nrf24l01,
                //and 5 is the default, so just skip it
                //nrf24.set_address_width(5).unwrap();
                nrf24
                    .set_rx_addr(
                        0,
                        &[
                            group.reverse_bits(),
                            b'u'.reverse_bits(),
                            b'b'.reverse_bits(),
                            b'i'.reverse_bits(),
                            b't'.reverse_bits(),
                        ],
                    )
                    .unwrap();
                nrf24
                    .set_pipes_rx_lengths(&[
                        Some(32),
                        Some(32),
                        Some(32),
                        Some(32),
                        Some(32),
                        Some(32),
                    ])
                    .unwrap();
                nrf24
                    .set_tx_addr(&[
                        group.reverse_bits(),
                        b'u'.reverse_bits(),
                        b'b'.reverse_bits(),
                        b'i'.reverse_bits(),
                        b't'.reverse_bits(),
                    ])
                    .unwrap();
                Ok(Standby(nrf24, group))
            }
            Err(e) => Err(e),
        }
    }
}
