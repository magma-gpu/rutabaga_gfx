<div align="center">
  <img src="https://github.com/magma-gpu/rutabaga_gfx/raw/chromeos/images/rutabaga_department_releases.png" alt="">
</div>

**WASHINGTON, DC** -- Today, the Rutabaga Department of Releases and Maintainence proposed a new
strategy to ensure stability for crosvm-on-ChromeOS.

The proposal is due to the recent
[focus on Android Desktop](https://www.theregister.com/2025/09/25/google_android_chromeos/), which
has reduced interest in work related to ChromeOS virtualization. However, many ChromeOS devices will
not migrate to Android Desktop, and will require
[10-years of updates](https://support.google.com/chrome/a/answer/6220366).

The crosvm team has chosen to continue updating crosvm in ChromeOS, and as a consequence, rutabaga
must be updated there too.

This presents several maintainence challenges for rutabaga:

1. A subtle change in rutabaga might break ChromeOS, and nobody has bandwidth to test refactors on a
   ChromeOS device
1. rutabaga would have to support features that need be deprecated (OpenGL, minigbm rather Mesa GBM)
   on account of ChromeOS.
1. New rutabaga features wouldn't be useful for ChromeOS, but would bring-in dependencies

Taking inspiration from Mesa3D's [Amber branch](https://docs.mesa3d.org/amber.html), the
department's proposal would _functionally_ freeze the rutabaga version used by ChromeOS via the
**chromeos** branch. The API would be the same for the *main* and *chromeos* branch.

The procedure is described as follows:

1. A new API is introduced in rutabaga *main*
1. A new *main* release is desired (say, *v0.4.2*)
1. A change is landed in rutabaga *chromeos* that stubs out the new API. The API would always return
   success or something else acceptable to crosvm.
1. *v0.4.2* and *0.4.2-chromeos* released at the same time on crates.io
1. Upstream crosvm uses *v0.4.2*, ChromeOS crosvm uses *v0.4.2-chromeos*.

This does require a small downstream changes to crosvm-on-ChromeOS's Cargo.toml file. A prototype
was done, which passes the
[ChromeOS CI](https://chromium-review.googlesource.com/q/topic:%22rutabaga-chromeos-attempt%22).

The proposal keeps ChromeOS stable, but always allows evolution of *main*.

In these bitterly-divided times, leaders on both sides of the aisle praised the prosposal. President
Donald Trump said removing ChromeOS code in rutabaga *main* would allow space for new luxury
ballrooms, and claimed he deserved a Github award for the effort. Former Vice President Kamala
Harris also welcomed the move, saying it presents a vision for
"[what rutabaga can be, unburdened by what has been](https://www.youtube.com/shorts/kFkchOYayUE)".

The proposal goes to crosvm maintainers for review, before a Congressional vote is held.
