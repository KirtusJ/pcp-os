//! The name a person reads for a program, where the loader's name for its file is a product's name
//! written the way a file is.
//!
//! The loader names files: `tailorbird`, `surfbird`. People read products: Tailorbird, Surfbird.
//! This says which words the list of programs and the pinned row show (not Details, which shows files). It changes nothing else:
//! the file's own name is what the kernel starts, says on the serial line, writes in the register
//! and shows in Details, beside the digest, so that a reviewer still reads exactly what the loader
//! handed over. It is also no claim of identity. Any file with that name is shown with that
//! spelling; which bytes they are is the digest's to say, and the picture beside a program is tied
//! to a digest for that reason (`pictures.rs`).
//!
//! Exact names only: `Tailorbird-old`, `tailorbird.elf` and `TAILORBIRD` are other files and are
//! shown as they are. No I/O here, so that the rule is tested on a host.

/// The loader's names that have a product's spelling, and that spelling.
pub const NAMES: [(&str, &str); 2] = [("tailorbird", "Tailorbird"), ("surfbird", "Surfbird")];

/// What the list of programs shows for the file the loader named `name`.
pub fn shown(name: &str) -> &str {
    for (file, product) in NAMES {
        if name == file {
            return product;
        }
    }
    name
}
