Calliope mini / nRF24L01 radio communication
============================================

The [Calliope mini](https://calliope.cc/) uses a radio protocoll
which superficially looks very similar to the one used by
[nRF24L01 modules](https://www.seeedstudio.com/blog/2019/11/21/nrf24l01-getting-started-arduino-guide/),
which can be bought very cheaply - at least if you are willing to buy
cheap clones.

As I have both a Calliope and some of those cheap modules,
of course I wanted to make them talk to each other. However,
I had to fight with some subtle differences. Please note
that I am quite sure my modules are clones, and may not
behave identical to the original. So some of the observations
may not be true for genuine modules from [Nordic](https://www.nordicsemi.com/).

While I don't have the hardware to verify this, I assume that
everything I write about the Calliope mini is also valid for the
very similar [BBC micro:bit](https://www.microbit.org/).

Tools
=====

On the Calliope side, I use the
[Open Roberta](https://lab.open-roberta.org/) visual
code editor to write a trivial radio firmware.

The nRF24L01 is connected to an STM32F103 microcontroller
on a [Blue pill](https://stm32duinoforum.com/forum/wiki_subdomain/index_title_Blue_Pill.html) board.
The firmware is written in Rust, using the [embedded-nrf24l01](https://crates.io/crates/embedded-nrf24l01)
crate.

Radio frequency
===============

First issue was that I was confused by a label in the
[Open Roberta](https://lab.open-roberta.org/) visual
code editor: There is a radio control block labeled `set
channel to`, and I naively assumed that it sets the radio
channel. But in fact it's translated to 
[`_uBit.radio.setGroup();`](https://lancaster-university.github.io/microbit-docs/ubit/radio/#setgroup),
which sets a part of an address used inside the radio packets.

The frequency used is just the default value, 2.407 GHz (channel 7).

Transmission mode
=================

This one was easy: Just use the 1 MBit/s transmission mode.

Addressing
==========

The Calliope mini uses the same addressing scheme as the micro:bit,
four bytes `0x75626974` derived from the string 'ubit' followed by the
group-id. ([1],[2]) Note that the 32 bit number is stored little endian,
so the byte `0x74` for 't' comes first. So the string really is `"tibu\x00"`.

Setting this value directly on the nRF24L01 doesn't work, though.

Using a method [described by Travis Goodspeed](https://travisgoodspeed.blogspot.com/2011/02/promiscuity-is-nrf24l01s-duty.html),
I was able to capture packets without knowing the address.

Looking at the observed packets, it seems like somewhere the bit order
gets reversed. I'm not sure if this is caused by a bug. More likely, it
happens because the radio protocols are simply defined differently:

Section 17.1.2 of the [nRF51 data sheet](https://infocenter.nordicsemi.com/pdf/nRF51_RM_v3.0.pdf),
which describes the microcontroller used by Calliope mini, says that it
sends the address LSB first. Note that the payload is also sent LSB first,
while the CRC is sent MSB first. We will need that later.

Section 7.10.1 of the [nRF24L01 data
sheet](https://cdn.sparkfun.com/datasheets/Wireless/Nordic/nRF24L01_Product_Specification_v2_0.pdf)
suggests that the address (as well as all other fields) are sent MSB
first.

In the end, I had the write the bytes in bit-reversed order to the nRF24L01 registers,
to get the packets recognized.

And hooray, with CRC disabled, I do receive data packets!

Whitening
=========

Now that I receive data packets, the next step is interpreting the
received data frame. The sending side uses a feature called data whitening,
as described in section 17.1.6 of the [nRF51 data sheet](https://infocenter.nordicsemi.com/pdf/nRF51_RM_v3.0.pdf).
The initialization vector DATAWHITEIV is set to `0x18` ([3]), with the additional twist that
the bits are reversed and the chip automatically sets the bit at position 0 of the LSFR (Table 129 of the data sheet).

After whitening, the data bits need to be reversed, as mentioned above.

With this implemented, clear text data becomes visible: The first byte contains the length of the data packet
(excluding address and CRC), followed by three bytes I didn't decode yet, followed by the payload as written
with the `send message` block of Open Roberta. The last two bytes are the CRC checksum.

CRC
===

While this is sufficient for receiving data, it would be nice if the recipent could verify the checksum. And for sending
data, it is necessary to generate a valid CRC, otherwise the receiving Calliope mini will just discard the packet.

The variant of CRC used has a truncated polynom of `0x1021` and an initial value of `0xffff`. It is implemented
in a `no-std` friendly way by the Rust crate [crc16](https://crates.io/crates/crc16) as variant `CCITT_FALSE`.
Again, bit-ordering becomes important: While a CRC itself is defined on a bitstream, when implementing it in terms of
bytes, the right bit order needs to be chosen. The nRF51 chip sends the data LSB first, and that's also the ordering
used when calculating the CRC. But the mentioned CRC algorithm expects the MSB to be sent first, so we have to write
bit-reversed data to the CRC algorithm.

[1]: https://github.com/lancaster-university/microbit-dal/blob/7aedfab59ac74cf74d8ec906f3aab9f5bcb1e6af/inc/drivers/MicroBitRadio.h#L65
[2]: https://github.com/lancaster-university/microbit-dal/blob/7aedfab59ac74cf74d8ec906f3aab9f5bcb1e6af/source/drivers/MicroBitRadio.cpp#L303-L308
[3]: https://github.com/lancaster-university/microbit-dal/blob/7aedfab59ac74cf74d8ec906f3aab9f5bcb1e6af/source/drivers/MicroBitRadio.cpp#L335
