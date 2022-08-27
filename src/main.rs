use std::net::UdpSocket;

mod types;
use types::*;

/// The IP and port we &bind to
const BIND_ADDR: &str = "192.168.10.1:69";
/// The size we give our empty buffers by default, code should truncate to correct size
const BUFFER_SIZE: usize = 1500;
/// The file we are serving
const STAGE0: &[u8] = include_bytes!("D:/Code/Rust/AzphOS/bootloader/build/stage0.bin");

fn main() {
    let socket = UdpSocket::bind(BIND_ADDR).expect("Cannot bind");

    loop {
        let mut buf = [0; BUFFER_SIZE];

        let (len, src) = socket.recv_from(&mut buf).unwrap();
        println!("Received {len} byte(s) from {src:?}");

        let now = unsafe { core::arch::x86_64::_rdtsc()}; 
        let tftp = TFTP::parse(&buf, len);
        println!("Cycles: {}", unsafe{ core::arch::x86_64::_rdtsc() - now } );
        println!("{tftp:?}");
        if let Some(tftp) = tftp {
            match tftp.opcode {
                Opcode::Read => {
                    let data_len = STAGE0.len();
                    //let mut blk_sz = tftp.blksize.unwrap_or(512);
                    let mut blk_sz = 512;

                    for (blk_ctr, blk_start) in (0..data_len).step_by(blk_sz).enumerate() {
                        let mut buf: [u8; 1500] = [0u8; 1500];

                        if blk_start + blk_sz < data_len {
                            let len = tftp.data(&mut buf, blk_start, blk_sz, blk_ctr);
                            socket.send_to(&buf[..len], src).unwrap();
                        }else{
                            blk_sz = data_len - blk_start;
                            let len = tftp.data(&mut buf, blk_start, blk_sz, blk_ctr);
                            socket.send_to(&buf[..len], src).unwrap();
                        }
                        std::thread::sleep(std::time::Duration::from_millis(15))
                    }
                    
                },
                _=> {}
            }
        }
    }
}


#[allow(dead_code)]
#[derive(Debug)]
struct TFTP<'tftp>{
    opcode: Opcode,
    fname: &'tftp str,
    typ: Typ,
    blksize: Option<usize>,
    tsize: Option<usize>,
}

impl<'tftp> TFTP<'tftp>{
    /// Reads an incomming TFTP packet and converts to a [TFTP] struct
    fn parse(buf: &'tftp [u8], len: usize) -> Option<Self> {     
        let mut data_ptr = 0;
        let opcode = match Opcode::try_from(&buf[..2]){
            Ok(op) => op,
            _ => return None
        };
        data_ptr += 2;

        // Its a null terminated string
        let fname = core::str::from_utf8(
            &buf[data_ptr..].splitn(2, |i| *i == 0x00)
            .next()
            .unwrap_or_default()
        ).unwrap_or_default();
        data_ptr += fname.len() + 1;

        let typ = match Typ::try_from(&buf[data_ptr..]){
            Ok(typ) => typ,
            _=> return None
        };
        data_ptr += typ.len() + 1;

        let options_raw = &mut buf[data_ptr .. len-1].split(|i| *i == 0x00);
        let mut blksize = None;      
        let mut tsize = None;      
        while let Some(key) = options_raw.next(){
            if let Some(value) = options_raw.next() {
                match Options::parse(key, value){
                    Options::Blksize(sz) => blksize = Some(sz),
                    Options::Tsize(sz) => tsize = Some(sz),
                    _=> continue
                }
            }else{
                break;
            }
        }
        Some(Self {
            opcode,
            fname,
            typ,
            blksize,
            tsize,
        })
    }
    /// This function will generate a data packet
    fn data(&self, buf: &mut [u8], blk_start: usize, blk_sz: usize, blk_ctr: usize) -> usize { 
        let header_len = 4;
        buf[..2].copy_from_slice(&Opcode::Data.serialise());

        // FIX ME!
        buf[2..header_len].copy_from_slice(&[0, (blk_ctr + 1) as u8]);

        buf[header_len..header_len+blk_sz].copy_from_slice(&STAGE0[blk_start .. blk_start + blk_sz]);

        blk_sz + header_len
    }
}
