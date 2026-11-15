# Testing which cryptographic check rejected an input

The proposed Section 4 change applies when a finding attributes rejection to one particular check. In the ML-KEM-512 example, a key with the wrong length can be rejected before the modulus check. A correctly sized pair differing in one packed coefficient tests that distinction. FIPS 203 Section 7.2 specifies both predicates. NIST documented a release in which its labeled adverse vectors could all be rejected on length. [FIPS 203](https://csrc.nist.gov/pubs/fips/203/final) · [NIST ACVP fixture repair](https://github.com/usnistgov/ACVP-Server/issues/460)

## Reproduce the control

Download NIST's two public input files to different local paths. The replay verifies their SHA-256 digests before inspecting them. [Historical file at c924096a](https://raw.githubusercontent.com/usnistgov/ACVP-Server/c924096a71e5d050742e31efa6846d1e2d6fb3bd/gen-val/json-files/ML-KEM-encapDecap-FIPS203/internalProjection.json) · [Repaired file at ad33b3d9](https://raw.githubusercontent.com/usnistgov/ACVP-Server/ad33b3d9504491767f1aa76382464f3b3fa2359e/gen-val/json-files/ML-KEM-encapDecap-FIPS203/internalProjection.json)

~~~sh
python3 check_crypto_predicate_evidence.py historical.json corrected.json
~~~

The five historical negative ML-KEM-512 inputs were 1216 bytes each with zero out-of-range coefficients in the first 768 bytes. All five repaired inputs were 800 bytes with one out-of-range coefficient. Starting with a public valid key from the repaired release, the replay makes three cases. These contain coefficient 3328, coefficient 3329, and the first key truncated to 799 bytes. A size-only model accepts both 800-byte inputs. The fixture inspection and model mutation establish what those cases can distinguish.

Append `--openssl /path/to/openssl` to exercise an actual OpenSSL binary. The replay uses a syntactically complete public-key wrapper for each input, including the truncated case, and selects the default provider. OpenSSL 3.5.7 on Msys-x86_64 returned status codes 0, 0, 1, 1 for the unmodified published key, the 3328 case, the 3329 case and the truncated case. Both rejected cases produced the same key-loading diagnostic. The tested openssl.exe had SHA-256 78340fb01dea2df5caab73dcf28833b31329226a4c9e02f717f1d399592265b9 and its msys-crypto-3.dll had SHA-256 01e77bdd11caaef53315b98295a0a3cfd439b103d5832ba48146a58b389f0995. These are observations at the public-key loading and checking interface. They do not trace the provider's internal branches or measure an encapsulation, timing property or deployment. [Public-key encoding in RFC 9935](https://www.rfc-editor.org/rfc/rfc9935.html)

## Apply the source edit for a successor packet

Start from an extracted copy of the build-22 Implementation packet pinned in [the edition assembler](../assemble_release.py). Its archive SHA-256 is `c7310f55b34ec7705b48363b95fc0839e7df96dd044c41bfd9b3183edf21abe4` and its manifest SHA-256 is `7ff72eed9162e2c11fc62f6681c4afbc29524bfa96b87d45a455423d027d4707`. The public root packet in the [build-19 edition](https://github.com/KirtusJ/pcp/blob/edb7487dd40215de7e238c046e73c6997c0a6e40/README.md) is an earlier source.

~~~sh
python3 edit_crypto_predicate_evidence.py /path/to/extracted/ACR_1.0_Implementation
~~~

The edit checks the exact predecessor specification and writes only that Markdown source. Its revised SHA-256 is `37a537449bb797ce6f3beed676f452336227ce5700451a9751294a2f7b096d56`. In the publisher's [method build](build_method.sh) this edit precedes assembly phases A and B, which regenerate the catalog, clause indexes, provenance, verifier and manifest. Rebuild reading copies, editions, host and plugin pins together when publishing that successor.
