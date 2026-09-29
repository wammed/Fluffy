# Third-Party Licenses & Runtime Audit Record

<p align="center">
  <a href="THIRD_PARTY_LICENSES.md">English</a> | <a href="THIRD_PARTY_LICENSES.ja.md">日本語</a> | <a href="../docs/PORTAL.md">📚 Documentation Portal</a> | <a href="../README.md">🏠 Root README</a>
</p>

## 1. Purpose

This document records licensing and provenance information relevant to the Fluffy
project.

Fluffy is distributed as source code. It does not bundle or redistribute the external
multimedia runtimes described below. Users and distributions provide their own
runtime packages.

This document separates:

1. Fluffy itself
2. Rust/Cargo dependencies
3. External GStreamer runtime components
4. External FFmpeg runtime components
5. AI-generated artwork/assets
6. Downstream packaging

This is a project audit record, not legal advice.

---

## 2. Fluffy

### License

**MIT License**

The project declares:

```toml
license = "MIT"
```

The repository contains the project's `LICENSE` file with the MIT License.

Fluffy source code is not licensed under GPL merely because an external FFmpeg
installation used by Fluffy may contain GPL-licensed components.

### Distribution model

The official project distribution policy is source-only:

- No official binary releases
- No bundled AppImage
- No bundled FFmpeg
- No bundled GStreamer runtime
- Users/distributions build and package Fluffy for their target environment

---

## 3. Rust / Cargo Dependencies

Fluffy uses Cargo for Rust dependency management.

Dependency licensing was audited with `cargo-deny`.

### cargo-deny result

The current audit result is:

- Advisories: **OK**
- Bans: **OK**
- Licenses: **OK**
- Sources: **OK**

The configured allowed licenses are:

- MIT
- Apache-2.0
- Apache-2.0 WITH LLVM-exception
- ISC
- Unicode-3.0
- Zlib

Unknown registries and unknown Git sources are denied by the current `deny.toml`
configuration.

### Duplicate crate versions

`cargo-deny` reports a warning for multiple versions of `syn`:

- `syn 2.0.119`
- `syn 3.0.6`

This is a transitive dependency situation, not a direct Fluffy dependency.

The current configuration sets multiple versions to `warn`, not `deny`.
No license failure was reported.

---

## 4. GStreamer

GStreamer is an external runtime dependency used by Fluffy for video playback.

Fluffy does not bundle GStreamer.

### 4.1 Audited CachyOS packages

The development/test environment is CachyOS using packages from `cachyos-extra-v3`.

The following packages were inspected with `pacman -Qi`:

| Package | Version | Reported license |
|---|---:|---|
| `gstreamer` | 1.28.7-2.1 | LGPL-2.1-or-later |
| `gst-plugins-base` | 1.28.7-2.1 | LGPL-2.1-or-later |
| `gst-plugins-good` | 1.28.7-2.1 | LGPL-2.1-or-later |
| `gst-plugins-bad` | 1.28.7-2.1 | LGPL-2.1-or-later |
| `gst-libav` | 1.28.7-2.1 | LGPL-2.1-or-later |

The installed `gst-launch-1.0` and `gst-inspect-1.0` executables are provided by
the `gstreamer` package.

`gst-libav` depends on the externally installed `ffmpeg` package.

### 4.2 Audited GStreamer plugins

The following runtime components were inspected with `gst-inspect-1.0` in the
same test environment:

| Component | Version | Reported license |
|---|---:|---|
| `waylandsink` | 1.28.7 | LGPL |
| `h264parse` | 1.28.7 | LGPL |
| `qtdemux` | 1.28.7 | LGPL |
| `decodebin` | 1.28.7 | LGPL |
| `nvh264dec` | 1.28.7 | LGPL |
| `avdec_h264` | 1.28.7 | LGPL |

These checks document the inspected runtime components in the audited environment.
They are not a guarantee that every possible GStreamer plugin installation has the
same licensing.

### 4.3 Runtime distribution

Users/distributions obtain GStreamer from their operating system or package
repository. Fluffy does not redistribute the inspected GStreamer binaries or
plugins.

The package-level and plugin-level results above describe the specific CachyOS
environment used for the project audit.

---

## 5. FFmpeg

FFmpeg is used as an external process for media inspection and conversion.

Relevant external commands include:

- `ffprobe`
- `ffmpeg`

The conversion path uses:

```text
-c:v libx264
```

Therefore the installed FFmpeg runtime must provide `libx264` support for that
conversion path.

### 5.1 Audited CachyOS package

The audited environment is:

- OS: CachyOS
- Package source: `cachyos-extra-v3`
- Package: `ffmpeg`
- Package version: `2:9.0.2-1.1`
- FFmpeg version: `n9.0.2`
- Architecture: `x86_64_v3`

The installed package reports:

```text
Licenses : GPL-3.0-only
```

The FFmpeg build configuration includes:

```text
--enable-gpl
--enable-libx264
```

The installed package also depends on the `x264` package.

`ffmpeg -L` reports that this FFmpeg build is distributed under the GNU General
Public License.

### 5.2 Important licensing distinction

The GPL licensing of this installed FFmpeg runtime does **not** change the license
of Fluffy itself.

The relationship is:

```text
Fluffy (MIT)
    |
    +-- external process --> ffprobe / ffmpeg
                               |
                               +-- audited environment: GPL-3.0-only FFmpeg
                               +-- libx264
```

Fluffy's source distribution does not include this FFmpeg binary.

The licensing of an installed FFmpeg build is therefore an external runtime
consideration for the user's or distributor's environment.

Different distributions may provide FFmpeg builds with different configuration
choices and package metadata. The GPL-3.0-only result above is specifically the
CachyOS environment audited for this project.

### 5.3 Relationship with GStreamer

The audited `gst-libav` package reports:

```text
Licenses : LGPL-2.1-or-later
```

and declares the external `ffmpeg` package as a dependency.

These are separate package-level licensing records:

- `gst-libav`: LGPL-2.1-or-later
- `ffmpeg` in the audited CachyOS environment: GPL-3.0-only

The LGPL license reported by `gst-libav` should not be interpreted as changing the
license of the separately packaged FFmpeg runtime.

### 5.4 Separate consideration: codec/patent issues

Software licensing and codec/patent considerations are separate issues.

This document records the software licensing information observed for the tested
FFmpeg package. It does not provide a patent or codec legal assessment.

---

## 6. AI-Generated Artwork / Icons

Fluffy includes artwork created using an AI image-generation workflow.

The current project records identify:

- Base artwork generated with the Google Gemini app
- Model recorded by the project: **Nano Banana**
- Generation date recorded by the project: **2026-09-26**
- The generated base artwork was subsequently modified by a human
- A third-party similarity check has not been performed

Relevant project records:

- `IP_COMPLIANCE.md`
- `ICON_DESIGN_HISTORY.md`

No separate `ASSET_PROVENANCE.md` file was identified during the project audit.

### Handling policy

AI-generated artwork is tracked separately from software dependencies.

The project should not describe the AI-generated artwork as an ordinary third-party
software dependency or assign a software license to it solely because Fluffy itself
is MIT-licensed.

Provenance and applicable rights/terms should remain documented in the project's
asset/provenance records.

---

## 7. Third-Party Source Code / Vendored Code

The project audit found no dedicated:

- `vendor/`
- `third_party/`
- `licenses/`
- `LicenseS/`

directory containing copied third-party source code.

The project therefore does not currently appear to redistribute a vendored copy of
the external GStreamer or FFmpeg runtimes.

Cargo dependencies remain governed by their respective package licenses and are
tracked through the Cargo dependency audit.

---

## 8. Downstream Packaging

Fluffy is intended to be packaged by distributions or users rather than shipped
with an official bundled multimedia runtime.

Downstream packages should independently verify:

- the exact GStreamer plugins included or required by the package
- the exact FFmpeg package/build used by the package
- the licenses of any additional runtime components they redistribute
- any distribution-specific packaging obligations

A downstream package should not assume that the FFmpeg or GStreamer licensing
observed on the development machine is universally applicable to every distribution.

---

## 9. Audit Status

| Area | Status | Notes |
|---|---|---|
| Fluffy license | PASS | MIT |
| Cargo dependency licenses | PASS | `cargo-deny`: licenses OK |
| Cargo dependency sources | PASS | `cargo-deny`: sources OK |
| Cargo advisories | PASS | `cargo-deny`: advisories OK |
| Cargo duplicate versions | WARN | `syn` has multiple transitive versions |
| GStreamer package licenses | PASS* | Audited packages report LGPL-2.1-or-later |
| GStreamer plugin licenses | PASS* | Inspected plugins report LGPL |
| FFmpeg runtime | DOCUMENTED* | Tested CachyOS package is GPL-3.0-only |
| AI artwork provenance | DOCUMENTED* | Existing project records; similarity check not performed |
| Vendored third-party source | NONE FOUND | No dedicated vendor/third-party license tree found |

`*` These entries describe the audited/tested environment and project records, not a
universal statement about every possible user installation or asset.

---

## 10. Audit Evidence

The external runtime audit was performed on the CachyOS development/test
environment using:

```text
gst-launch-1.0 --version
gst-inspect-1.0 <plugin>
pacman -Qo "$(command -v gst-launch-1.0)"
pacman -Qo "$(command -v gst-inspect-1.0)"
pacman -Qi gstreamer
pacman -Qi gst-plugins-base
pacman -Qi gst-plugins-good
pacman -Qi gst-plugins-bad
pacman -Qi gst-libav

ffmpeg -L
ffmpeg -version
ffmpeg -buildconf
ffmpeg -encoders
pacman -Qo "$(command -v ffmpeg)"
pacman -Qi ffmpeg
```

The audit records the package and runtime state observed at the time of this
project review.

---

## 11. Recommended Maintenance

When dependencies or runtime requirements change, update this document if the change
affects licensing or distribution.

In particular, re-check:

1. `cargo deny check` after dependency changes
2. GStreamer package/plugin licensing when required components change
3. FFmpeg configuration when the conversion pipeline changes
4. AI asset provenance when new artwork is added
5. Downstream packaging requirements when official packaging targets are introduced

The license audit should remain separate from functional compatibility testing.
