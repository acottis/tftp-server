use std::path::Path;
use std::{
    io::BufRead,
    net::{ToSocketAddrs, UdpSocket},
};
mod types;
use types::*;

/// The IP and port we &bind to
const BIND_ADDR: &str = "192.168.10.1:69";
/// The size we give our empty buffers by default, code should truncate to correct size
const BUFFER_SIZE: usize = 1500;
/// The file we are serving
//const STAGE0: &[u8] = include_bytes!("D:/Code/Rust/AzphOS/bootloader/build/stage0.bin");
/// Blocksize as bytes
const BLKSIZE: [u8; 7] = [0x62, 0x6C, 0x6B, 0x73, 0x69, 0x7A, 0x65];

fn main() {
    env_logger::init();
    log::info!("TFTP Server starting...");
    let file_path = std::env::args().nth(1).unwrap_or(String::from(
        "D:/Code/Rust/AzphOS/bootloader/build/stage0.bin",
    ));
    if !Path::new(&file_path).try_exists().unwrap(){
        log::error!("Boot file {file_path} does not exist");
        log::warn!("\x1b[1;32mUsage: cargo r --release \"path-name-here\"\x1b[0m");
        panic!("Boot file {file_path} does not exist")
    }

    let socket = UdpSocket::bind(BIND_ADDR).expect("Cannot bind");
    log::info!("Listening on {BIND_ADDR:?}");

    loop {
        let mut buf = [0; BUFFER_SIZE];

        let (len, src) = socket.recv_from(&mut buf).unwrap();
        log::info!("Received {len} byte(s) from {src:?}");

        //let now = unsafe { core::arch::x86_64::_rdtsc() };
        let tftp = TFTP::parse(&buf, len);
        //println!("Cycles: {}", unsafe { core::arch::x86_64::_rdtsc() - now });
        if let Some(tftp) = tftp {
            match tftp.opcode {
                Opcode::Read => {
                    log::info!("TFTP read recieved from {src}");
                    handle_read(&socket, &src, &tftp, &file_path).or_else( |e| -> Result<(),()> {
                        log::error!("Failed to complete read request from {src}");
                        log::error!("{e}");
                        Ok(())
                    }).unwrap();
                },
                _ => { unimplemented!(); }
            }
        }
    }
}

fn handle_read(
    socket: &UdpSocket,
    src: impl ToSocketAddrs + Copy,
    tftp: &TFTP,
    file_path: impl AsRef<Path>,
) -> Result<(), std::io::Error> {
    let boot_file = std::fs::File::open(file_path)?;
    let mut reader =
        std::io::BufReader::with_capacity(1024 * 1024 * 32, boot_file);
    let data = reader.fill_buf()?;

    // If a custom block size is requested we must acknowledge
    let mut blk_sz = if let Some(blk_sz) = tftp.blksize {
        let mut buf: [u8; 20] = [0u8; 20];
        // Generate a response and send
        let len = tftp.options_acknowledge(&mut buf);
        socket.send_to(&buf[..len], src)?;

        // Check for ACK NOT IMPLEMENTED, WE ASSUME IT WORKED dont @ me
        socket.recv_from(&mut buf)?;

        blk_sz
    } else {
        512
    };

    let data_len = data.len();
    for (blk_ctr, blk_start) in (0..data_len).step_by(blk_sz).enumerate() {
        let mut buf: [u8; 1500] = [0u8; 1500];

        // Handle the last data packet
        if blk_start + blk_sz > data_len {
            blk_sz = data_len - blk_start;
        }
        let len = tftp.data(&mut buf, &data, blk_start, blk_sz, blk_ctr + 1);
        socket.send_to(&buf[..len], src).unwrap();

        // Check for ACK
        let mut buf: [u8; 100] = [0u8; 100];
        let (len, _) = socket.recv_from(&mut buf).unwrap();

        let res = TFTP::parse(&buf, len);
        if let Some(res) = res {
            if !res.ack_valid(blk_ctr + 1) {
                log::error!("Bad Ack: {} Received", blk_ctr + 1);
                return Ok(());
            }else{
                log::info!("Ack: {} Received, {blk_sz} Byte(s)", blk_ctr + 1);
            }
        } else {
            return Ok(());
        }

        // Handle the wierd edge case of our file being divisable by our block size
        if blk_start + blk_sz == data_len {
            let len = tftp.data(&mut buf, &data, 0, 0, blk_ctr + 2);
            socket.send_to(&buf[..len], src).unwrap();

            // Check for ACK
            let mut buf: [u8; 100] = [0u8; 100];
            let (len, _) = socket.recv_from(&mut buf).unwrap();

            let res = TFTP::parse(&buf, len);
            if let Some(res) = res {
                if !res.ack_valid(blk_ctr + 1) {
                    log::error!("Bad Ack: {} Received", blk_ctr + 1);
                    return Ok(());
                }else{
                    log::info!("Ack: {} Received, {blk_sz} Byte(s)", blk_ctr + 1);
                }
            } else {
                return Ok(());
            }
        }
    }
    log::info!("TFTP read finished, {data_len} Byte(s) Total");
    Ok(())
}

#[allow(dead_code)]
#[derive(Debug)]
struct TFTP<'tftp> {
    opcode: Opcode,
    fname: Option<&'tftp str>,
    typ: Option<Typ>,
    blksize: Option<usize>,
    tsize: Option<usize>,
    block: Option<u16>,
}

impl<'tftp> TFTP<'tftp> {
    /// Reads an incomming TFTP packet and converts to a [TFTP] struct
    fn parse(buf: &'tftp [u8], len: usize) -> Option<Self> {
        let mut data_ptr = 0;
        let opcode = match Opcode::try_from(&buf[..2]) {
            Ok(op) => op,
            _ => return None,
        };
        data_ptr += 2;

        if opcode == Opcode::Ack {
            let block = Some((buf[2] as u16) << 8 | buf[3] as u16);
            return Some(Self {
                opcode: Opcode::Ack,
                fname: None,
                typ: None,
                blksize: None,
                tsize: None,
                block,
            });
        }

        // Its a null terminated string
        let fname = core::str::from_utf8(
            &buf[data_ptr..]
                .splitn(2, |i| *i == 0x00)
                .next()
                .unwrap_or_default(),
        )
        .unwrap_or_default();
        data_ptr += fname.len() + 1;

        let typ = match Typ::try_from(&buf[data_ptr..]) {
            Ok(typ) => typ,
            _ => return None,
        };
        data_ptr += typ.len() + 1;

        let options_raw = &mut buf[data_ptr..len - 1].split(|i| *i == 0x00);
        let mut blksize = None;
        let mut tsize = None;
        while let Some(key) = options_raw.next() {
            if let Some(value) = options_raw.next() {
                match Options::parse(key, value) {
                    Options::Blksize(sz) => blksize = Some(sz),
                    Options::Tsize(sz) => tsize = Some(sz),
                    _ => continue,
                }
            } else {
                break;
            }
        }
        Some(Self {
            opcode,
            fname: Some(fname),
            typ: Some(typ),
            blksize,
            tsize,
            block: None,
        })
    }
    /// This function will generate a data packet
    #[inline(always)]
    fn data(
        &self,
        buf: &mut [u8],
        file_data: &[u8],
        blk_start: usize,
        blk_sz: usize,
        blk_ctr: usize,
    ) -> usize {
        let header_len = 4;
        buf[..2].copy_from_slice(&Opcode::Data.serialise());

        buf[2] = (blk_ctr >> 8) as u8;
        buf[3] = blk_ctr as u8;

        buf[header_len..header_len + blk_sz]
            .copy_from_slice(&file_data[blk_start..blk_start + blk_sz]);

        blk_sz + header_len
    }
    /// Acknowledge an options request
    #[inline(always)]
    fn options_acknowledge(&self, buf: &mut [u8]) -> usize {
        let mut buf_ptr = 0;
        buf[..2].copy_from_slice(&Opcode::OAck.serialise());
        buf_ptr += 2;
        buf[buf_ptr..buf_ptr + BLKSIZE.len()].copy_from_slice(&BLKSIZE);
        buf_ptr += BLKSIZE.len();
        buf[buf_ptr] = 0x00; // Null term
        buf_ptr += 1;
        let blk_sz = format!("{}\0", self.blksize.unwrap().to_string());
        buf[buf_ptr..buf_ptr + blk_sz.len()].copy_from_slice(blk_sz.as_bytes());

        buf_ptr + blk_sz.len()
    }
    /// Check if we got the ACK we wanted
    #[inline(always)]
    fn ack_valid(&self, blk_ctr: usize) -> bool {
        if let Some(blk_num) = self.block {
            if blk_num as usize != blk_ctr {
                log::error!(
                    "Something went wrong with order, we sent {} and recieved ack for {}",
                    blk_ctr, blk_num
                );
                return false;
            }
        }
        true
    }
}
