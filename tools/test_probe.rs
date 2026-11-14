extern crate alloc;
use rustls_pki_types::TrustAnchor;
use webpki::Error;

fn print_res(name: &str, res: Result<TrustAnchor, Error>) {
    let b = unsafe { core::slice::from_raw_parts(&res as *const _ as *const u8, core::mem::size_of_val(&res)) };
    println!("{:40}: {:02x?}", name, &b[..core::cmp::min(24, b.len())]);
}

fn main() {
    print_res("Err(BadDer)", Err(Error::BadDer));
    print_res("Err(SignatureAlgorithmMismatch)", Err(Error::SignatureAlgorithmMismatch));
    print_res("Err(TrailingData(Cert))", Err(Error::TrailingData(webpki::DerTypeId::Certificate)));
    print_res("Err(CertNotValidForName)", Err(Error::CertNotValidForName(webpki::InvalidNameContext {
        expected: rustls_pki_types::ServerName::try_from("host.test").unwrap(),
        presented: alloc::vec::Vec::new(),
    })));
}
