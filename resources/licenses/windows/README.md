# Windows dependency notice fallback

`clipboard-win 5.4.1` declares `BSL-1.0` but omits the license text from its crates.io archive.
`BSL-1.0.txt` preserves the original upstream LICENSE from the source revision recorded in that
archive's `.cargo_vcs_info.json`:

https://github.com/DoumanAsh/clipboard-win/blob/3b27cf2bfd1adcfa6e0264eb51c1025ddaf0f342/LICENSE

The notice collector uses this canonical Boost Software License text only for dependencies whose
SPDX declaration is exactly `BSL-1.0` and whose source archive lacks a license file. Each output
explicitly labels the fallback and retains package name, version, repository and authors. Original
notice files, where present, are copied without alteration. No license terms are changed.
