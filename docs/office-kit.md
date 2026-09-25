# Office Kit: what it is, what it cannot do, and what we do about it

Researched 25 September 2026 from vivo's own shipped site data (pc.vivoglobal.com bundle: official support matrix, help strings, download endpoints) and the Reskilll portal. Facts below are quoted from those sources unless marked otherwise.

## The finding that changes our setup

**Office Kit has no Linux client.** vivo publishes download endpoints for `/windows/` and `/mac/` only. Requirements, in vivo's own words: laptop on **"Windows 10 or later"** or **"macOS 10.14.6 or later"**, phone on **"OriginOS 6 or later"**.

Both of our laptops run Ubuntu. That matters twice:

1. **Office Kit usage is 10% of the score**, taken from HackTracker device data, not from what we claim.
2. **During Red Light, which is 10.5 of the 19 build hours, the portal says the laptop is closed as a build machine and "Office Kit is the only route between the two".** Without it, Red Light means the phone alone.

**Action before Saturday: get one Windows or macOS laptop into the team**, even a borrowed one. It does not need to be the build machine. It only needs to run the bridge, hold the mirror on screen, and carry files. Everything else stays on Ubuntu.

## What Office Kit does (official support matrix, iQOO number series)

| Feature | On an iQOO 15 | Notes |
|---|---|---|
| Screen mirroring | Yes | Phone UI live on the laptop, and you can control the phone from that window. Drag files in and out of the mirror |
| File transfer | Yes | Both directions, drag and drop, folders included. No published size limit |
| Super Clipboard | Yes | Text, images and GIFs, both directions |
| Notes sync | Yes | The only cloud feature; needs a vivo account |
| Remote PC | Yes, iQOO 15 and later | The *phone* controls the laptop. Needs the same account on both |
| Keyboard and mouse sharing | **No** | vivo X Fold only |
| Screen extension (phone as second display) | **No** | vivo X Fold only, Windows 1903+ |

Three ways to pair: automatic on the same network with the phone unlocked; QR code shown on the laptop; or over USB with developer mode and USB debugging enabled. The USB path uses the same debugging permission adb uses.

**Constraints that will bite on the day:**
- The phone must be **unlocked with the screen on** for auto connect and for notification handoff.
- Both devices must be **on the same network** for every connected feature.
- vivo's own caveat: mirroring quality and transfer speed **degrade under wireless interference**. A hall full of laptops is exactly that, so prefer the USB pairing path.
- Screen recordings, scrolling screenshots and videos **cannot** be pushed to the laptop. Use file transfer for a demo recording.

## There is no API

No SDK, no intent, no broadcast, no developer documentation. Searched vivo's developer site, the shipped bundle and community sources. **Our app cannot trigger Office Kit programmatically.** Plan accordingly: Office Kit is the visible human bridge around the product, not a component inside it. Anything our software needs to move between phone and laptop, it moves over our own link.

## What we rehearse on Ubuntu tonight

These are not Office Kit. They imitate individual functions so the demo motions are familiar.

| Office Kit function | Ubuntu stand-in | Command |
|---|---|---|
| Screen mirroring and control | scrcpy | `~/Downloads/scrcpy-linux-x86_64-v4.1/scrcpy` |
| Clipboard sharing, notifications | GSConnect (KDE Connect for GNOME) | `sudo apt install gnome-shell-extension-gsconnect` |
| File transfer over USB | adb | `adb push file /sdcard/Download/` |
| Drop a file with no pairing | LocalSend | flatpak, or the .deb from their releases page |

**The apt trap, confirmed on this machine:** `apt-cache policy scrcpy` offers version **1.25**, from 2022, which fails on recent Android. We installed the official **v4.1** release build instead.

## How we use it at the event

- Pair Office Kit at the **10:00 teach-in**, on the borrowed Windows or Mac laptop, over USB.
- Keep the mirror **connected and visible** for the whole event. Scoring is by duration and count, so a bridge switched on at hour 25 scores nothing.
- Use it for real work, not theatre: move the model file to the phone with it, mirror the phone dashboard during the demo so judges see the phone doing the work, and use the shared clipboard for pairing details.
- During Red Light it is legitimately the laptop keyboard driving the phone, which the portal describes as "type into the device at full speed".
