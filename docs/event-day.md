# Event day: Hyderabad City Battle, 26 to 27 September 2026

Taken from the Reskilll portal's own data (the schedule, block, rubric and venue records inside its site bundle) on 25 September 2026. Participant claims are marked as such. The Terms allow the organisers to change any of it, so confirm times in the WhatsApp group on the day.

## Where and when

- **Venue:** The Hive, Gachibowli, next to Sheraton, Financial District, Nanakramguda, Hyderabad 500032.
- **Report time:** the Hyderabad record says **09:00 Saturday**; the published schedule says check-in at **08:00**. Both are official, so plan for **08:00 to 08:30** and ask in the group.
- **Bring:** laptop, charger, government photo ID. Also, not on their list but obvious from the venue: power strip, long extension, power bank, and your own mobile data.
- **On-site contacts:** Rahul +91 93533 17113, Sam +91 73385 44449.

## The schedule

| Day | Time | What |
|---|---|---|
| Sat | 08:00 | Check-in: ID, NDA, loaner phone, desk. Breakfast 08:30 to 09:30 |
| Sat | 10:00 | Clock starts. Keynote, jury and mentor intros, Green and Red reveal, **HackTracker and Office Kit teach-in**, rules and rubric |
| Sat | 11:00 | Building starts |
| Sat | 15:30 | Mentor round 1, parallel panels, building continues |
| Sat | 19:00 to 22:00 | **Evaluation round 1.** Scored, no elimination |
| Sun | 09:00 to 12:00 | **Evaluation round 2.** Judged at tables, **demo on the iQOO phone**. Building continues until 12:00 |
| Sun | 13:30 | Top 10 per bucket announced. Pitches from 13:45, **3 to 5 minutes, demo on hardware** |
| Sun | 16:15 | Awards, including "Most iQOO Usage". Top 3 students and top 3 professionals advance to the finale |

## Red and Green blocks

| Colour | Day | Window | Hours |
|---|---|---|---|
| Green | Sat | 11:00 to 13:00 | 2.0 |
| **Red** | Sat | 13:00 to 15:30 | 2.5 |
| Green | Sat | 15:30 to 16:30 | 1.0 |
| **Red** | Sat | 16:30 to 19:00 | 2.5 |
| Eval | Sat | 19:00 to 22:00 | 3.0 |
| **Red** | Sat | 22:00 to 00:00 | 2.0 |
| **Red** | Sun | 00:00 to 01:00 | 1.0 |
| **Green** | Sun | **01:00 to 06:30** | **5.5** |
| **Red** | Sun | 06:30 to 09:00 | 2.5 |
| Eval | Sun | 09:00 to 12:00 | 3.0 |

**8.5 hours Green, 10.5 hours Red**, plus 6 hours of evaluation.

**The planning fact that decides our weekend: 5.5 of our 8.5 laptop hours are between 01:00 and 06:30 on Sunday night.** Two people, so sleep in shifts. Sleeping through that block costs two thirds of all remaining laptop time.

During Red the laptop is closed as a build machine and Office Kit is the sanctioned bridge, so Red hours go to phone-side work: device testing, thermal runs, dashboard use, rehearsal, pitch writing.

## Judging

| Weight | Criterion | Scored by |
|---|---|---|
| 30% | End product quality: does it work, is it useful, would someone keep using it | Jury |
| 20% | Novelty and impact | Jury |
| 15% | Creative phone use: camera, voice, on-device AI | **Device telemetry** |
| 15% | Technical depth: architecture, code quality, robustness, real use of the hardware | Jury |
| 10% | Office Kit usage | **Device telemetry** |
| 10% | Demo and presentation | Jury |

75% jury, 25% telemetry. HackTracker records **counts and durations only**, no keystrokes or screenshots.

**Hyderabad jury:** Prabhakar Daley, Principal Architect at LTIMindtree, described as bringing "a sharp, data-informed perspective... challenging teams to think deeper". Three more were announced: Amit Pahwa, Krishna Gangadhar (Genpact), Suman Nandamury (Colley). Thirteen mentors including engineers from Apple, Verizon, Bank of America, Deliveroo and HighRadius. Expect questions about evidence and architecture rather than vision, which suits us: our numbers are measured and our limits are stated.

**Scale, from participant posts, unverified:** roughly 110 to 130 people and 50 to 60 teams per city, about 27 teams per bucket, so the Top 10 is roughly a one in three chance.

## The build rules

The guide: **"Original work only: code written during the event window. No shipping a pre-built product."** and **"Open-source libraries and frameworks are fine with attribution; carrying in a completed app is not."** Organisers **"may verify a project was built inside the event window"**, which in practice means the git history.

The Terms are slightly wider: **"Submissions must be the original work of the team, created during the Event (pre-Event drafting of ideas is permitted)."**

**What we do, plainly:**
- Fresh repository, first commit after 10:00 on Saturday, frequent timestamped commits.
- A README listing every third-party component with its licence: llama.cpp and ggml (MIT), the model weights (Apache 2.0), Android libraries.
- The disclosure checkbox is honest.
- Architecture, model choice, flags, and the pitch are "drafting of ideas" and travel with us as notes.

## Submission

Repo and demo assets go to the Reskilll platform **before a hard cutoff, with repos locked before the Top 10 pitches**. The exact cutoff time is not published; building continues to 12:00 on Sunday and pitches start at 13:45, so treat **Sunday 12:00** as the deadline and confirm it at the teach-in.

## Three questions nobody has answered publicly

Ask these in the WhatsApp group before Saturday:
1. What exactly is the hard submission cutoff time?
2. Are laptops allowed at the judging table during evaluation rounds?
3. What is the policy on personal hotspots? We need our own link for the demo, and venue Wi-Fi measured too slow for splitting.

## One trap

A Reskilll blog titled "how to prepare for iQOO Hackathon 2026" ranks well on search and gives a confident hour-by-hour timeline. **It describes a different event**: an 8 hour hackathon in Gurugram in June, with a different rubric. Ignore it and use the portal.
