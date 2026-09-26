# The Red Light playbook

**The problem in one line:** of 19 hours of build time, only **8.5 are Green** (laptop allowed) and
**10.5 are Red** (laptop closed, everything through the phone via Office Kit). All compiling needs a
laptop. So Green is the scarce resource and Red must never waste it.

**The rule that follows:** Red Light produces **decisions, measurements, artifacts and rehearsal**.
Green Light produces **code only**. Walk into every Green block with a written list of exactly what
to type. Walk out of every Red block with the numbers and decisions the next Green block needs.

A second rule, learned last night: **never start something in Green you cannot finish in Green.**
A half-built change that cannot compile for two and a half hours is worse than nothing.

## The blocks

| Block | Window | Hours | What it is for |
|---|---|---|---|
| Green 1 | Sat 11:00 to 13:00 | 2.0 | First build: get something running end to end |
| **Red 1** | Sat 13:00 to 15:30 | 2.5 | First device testing, link decision, model decision |
| Green 2 | Sat 15:30 to 16:30 | 1.0 | Fix what Red 1 found. Mentor round runs in parallel |
| **Red 2** | Sat 16:30 to 19:00 | 2.5 | Thermal and memory limits, rehearse for Evaluation 1 |
| Eval 1 | Sat 19:00 to 22:00 | 3.0 | Scored checkpoint, no elimination |
| **Red 3** | Sat 22:00 to Sun 01:00 | 3.0 | Long unattended runs, pitch writing, sleep in shifts |
| Green 3 | Sun 01:00 to 06:30 | **5.5** | **The big one. Two thirds of all laptop time** |
| **Red 4** | Sun 06:30 to 09:00 | 2.5 | Final rehearsal, charge everything, insurance recordings |
| Eval 2 | Sun 09:00 to 12:00 | 3.0 | Judged at tables, demo on the phone. Build until 12:00 |

Times are indicative; the organisers may change them. The structure will not change.

## Standing tasks, every Red block

1. **Keep Office Kit connected and mirroring.** It is scored by duration, from device telemetry, so
   a bridge switched on at hour 25 scores nothing. Assign it to one person as a standing job:
   check it is still connected at the top of every block.
2. **Keep the MeshAI app in the foreground with the screen on.** Android hands background apps the
   slow cores and throttles the radio. Measured: pinning to the big cores is worth about 30%, and
   the foreground app gets that for free.
3. **Keep the phone charging.** Every measurement should be on a charging device, or the numbers
   are not reproducible and the planner may refuse the device below 20%.
4. **Write every number down as you take it**, in the log format at the end of this file. Numbers
   without their conditions are worthless in a pitch.
5. **Use the phone for real AI work during Red.** The phone hosting the model is both genuinely
   useful and exactly what "creative phone use" telemetry is measuring: camera for QR pairing,
   voice if built, on-device inference throughout.

## Red 1, Saturday 13:00 to 15:30

The purpose of this block is to **settle the two decisions everything else depends on**: which link
we use, and which model we demo.

**Before the laptop closes at 13:00**, have Green 1 leave you with: the service running, the app
installed on both loaner phones, and both phones paired.

1. **Loaner phone setup, both of them.** Developer options on, USB debugging on, install via USB on,
   verify apps over USB off. Battery optimisation off for the app, autostart on, screen timeout at
   maximum, app locked in recents. Note the vendor id and get the udev rule in place, because the
   loaner will differ from any phone you tested on.
2. **The link matrix.** Measure all three, in this order, recording round trip average and worst
   case, then tokens per second on an identical prompt:
   - USB tethering from the phone's settings
   - the phone's own hotspot with the laptop joined to it
   - venue Wi-Fi, purely to document how bad it is
   Expect tethering near 0.8 ms and Wi-Fi in the tens with spikes. **Pick the winner and write it
   down as the demo configuration.** Do not revisit this later.
3. **Free memory on the loaner.** Note total and available with everything closed. This decides
   which models are possible, and whether one phone or two.
4. **The layer sweep.** On the chosen link, run 6, 10, 15 and 20 layers on the phone and record the
   speed of each. Last night the curve was flat on a fast link and sharply peaked on a slow one.
5. **Phone as host.** Record its tokens per second alone. This is the headline demo and it does not
   depend on the split being fast.
6. **Test the phone-to-phone link.** With two 16 GB phones the strongest demo is a model running
   across both with the laptop only coordinating. That needs the phones to reach each other: one
   phone's hotspot with the other joined to it, or both on a laptop hotspot. Measure the round trip
   between them before designing the demo around it.
7. **Write the Green 2 task list.** One and a half pages at most: exact files, exact changes.

## Red 2, Saturday 16:30 to 19:00

The purpose is **limits and rehearsal**, because Evaluation 1 starts at 19:00.

1. **The ten-minute sustained run.** Keep the phone generating for ten minutes and record tokens per
   second at minute 1, 3, 5, 10, with temperature and battery. Phones throttle in steps; this is the
   graph that shows you understand the hardware, and nobody else will have it.
2. **The memory ceiling.** Raise the phone's share until Android kills the worker, and record where
   that happened against what the planner predicted. If the planner's reserve is wrong, this is the
   evidence.
3. **CPU against GPU on the loaner.** The Adreno path is worth testing precisely because it is not
   obviously faster: on an 8 Elite, GPU decode measured *slower* than CPU. Either answer is a good
   answer, as long as it is measured.
4. **Rehearse the demo twice, out loud, on the phone.** Time it. The judging round is at a table
   with the demo on the phone, not on a projector.
5. **Prepare the three questions you will be asked**: why not just use the cloud, why not just buy a
   better laptop, and how do you know the phone is really doing the work. Answers are in
   `05-pitch-and-defence.md`. Have the "phone holds 500 MB" proof ready on screen.

## Red 3, Saturday 22:00 to Sunday 01:00

The purpose is **unattended work and words**, and sleep.

1. **Start a long run and leave it.** An overnight or half-hour batch job gives you a drain and
   thermal curve you cannot get any other way, and keeps telemetry accumulating.
2. **Record insurance footage.** Short clips on the phone of: pairing, the plan appearing, the
   phone's layers filling, an answer streaming, and the internet being switched off mid-answer.
   If anything fails on Sunday you still have proof. Note that screen recordings cannot be pushed
   through Office Kit, so transfer them as files.
3. **Write the pitch.** Three to five minutes. Use the laptop keyboard through Office Kit remote
   control to type into the phone; that is legitimate Red Light work and it accumulates bridge time.
4. **Write the README and the attribution list**, which the submission needs: llama.cpp and ggml
   (MIT), model weights (Apache 2.0), Android libraries, Office Kit.
5. **Plan Green 3 in detail.** It is 5.5 hours, two thirds of all remaining laptop time. Go in with
   a numbered list. Decide now who sleeps first; do not both sleep through it.

## Red 4, Sunday 06:30 to 09:00

The purpose is **being ready at 09:00**, nothing else. No new work.

1. Charge everything to full. Phones, laptops, power bank.
2. Reset the phones to a clean state: close every app, confirm free memory, confirm the app is in
   front and locked in recents.
3. Run the demo end to end twice, once with the internet off.
4. Confirm the fallbacks work: if the split is slow, can you switch to phone-as-host in under a
   minute? If a phone drops, does it rejoin?
5. Check the submission: repository link, demo assets, attribution, and confirm the cutoff time with
   an organiser. Building continues until 12:00, but repos lock before the pitches.
6. Final screenshots for the deck: the plan table, the phone dashboard, the measurement log.

## What Red Light cannot do

Be honest with yourselves about this, so you do not waste a block discovering it:

- No compiling, so no code changes, no new APK, no service rebuild.
- No model downloads onto the laptop.
- No dependency installs.

If you find a bug at 13:30, you write it down and fix it at 15:30. That is the whole discipline.

## The measurement log

Keep this in one file and fill a row every time you measure anything. A pitch with a table like this
beats a pitch with adjectives.

```
| time | what | link | model | layers on phone | tok/s | TTFT | phone temp | battery | note |
|------|------|------|-------|-----------------|-------|------|-----------|---------|------|
| 13:40| split| tether| 0.6B  | 15 of 28        | 8.3   | 1.2s | 38C       | 88% chg | quiet room |
```

## The Office Kit checklist

It runs only on Windows or macOS, so this belongs to whoever has that machine.

- Paired at the teach-in, over USB, before 11:00.
- Mirror open and visible for as much of the event as possible.
- Move the model file to the phone with it at least once, and say so in the pitch.
- Use the shared clipboard for pairing details instead of retyping.
- Use remote control for all Red Light typing.
- Mirror the phone dashboard onto the laptop during the demo, so judges see the phone working
  without crowding the table.
