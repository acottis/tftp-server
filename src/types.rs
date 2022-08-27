
#[derive(Debug, Clone, Copy)]
pub enum Options{
    Blksize(usize),
    Tsize(usize), // Size of file being transfered
    None,
}

impl Options {
    pub fn parse(key: &[u8], value: &[u8]) -> Self{
        match key {
            // Blksize
            [0x62, 0x6C, 0x6B, 0x73, 0x69, 0x7A, 0x65] => {
                let sz = match core::str::from_utf8(&value){
                    Ok(sz_str) => {
                        match sz_str.parse::<usize>(){
                            Ok(sz) => sz,
                            _=> return Self::None
                        }
                    }
                    _=> return Self::None
                };
                return Self::Blksize(sz)
            }
            // tsize
            [0x74, 0x73, 0x69, 0x7A, 0x65] => {
                let sz = match core::str::from_utf8(&value){
                    Ok(sz_str) => {
                        match sz_str.parse::<usize>(){
                            Ok(sz) => sz,
                            _=> return Self::None
                        }
                    }
                    _=> return Self::None
                };
                return Self::Tsize(sz)
            }
            _=> {
                println!("Unknown Option");
                println!("Parsing key:{key:X?}, value: {value:X?}");
                return Self::None
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Opcode{
    Read,
    Write,
    Data,
    Ack,
    Err,
}

impl TryFrom<&[u8]> for Opcode {
    type Error = ();
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value[..2]{
            [0,1] => { Ok(Self::Read) },
            [0,2] => { Ok(Self::Write) },
            [0,3] => { Ok(Self::Data) },
            [0,4] => { Ok(Self::Ack) },
            [0,5] => { Ok(Self::Err) },
            _ => { Err(()) }
        }
    }
}

impl Opcode {
    pub fn serialise(&self) -> [u8; 2] {
        match self{
            Self::Read  => [0,1], 
            Self::Write => [0,2],
            Self::Data  => [0,3],
            Self::Ack   => [0,4],
            Self::Err   => [0,5],
        }
    }
}

#[derive(Debug)]
pub enum Typ{
    Octet,
}

impl TryFrom<&[u8]> for Typ{
    type Error = ();
    
    #[inline(always)]
    fn try_from(buf: &[u8]) -> Result<Self, Self::Error>{
        let typ_slice = buf.splitn(2, |i| *i == 0x00).next();
        match typ_slice{
            Some([0x6f,0x63,0x74,0x65,0x74]) => { Ok(Self::Octet) },
            _=> return Err(()),
        }
    }
}

impl Typ {
    pub fn len(&self) -> usize {
        match self {
            Self::Octet => 5,
        }
    }
}