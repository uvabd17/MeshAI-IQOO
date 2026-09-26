# iQOO Hackathon 2026: Intel Report (team Maynards)

Compiled 22 Sep 2026. Sources: the official portal text (pulled from the iqoo.reskilll.com site bundles), iQOO, Reskilll and participant posts on X, Instagram and Reddit, press coverage, and LinkedIn.
VERIFIED = quoted from the portal or an official iQOO/Reskilll account. UNVERIFIED = from participants or aggregators only.

## 1. The basics
- **Organisers:** iQOO (vivo Mobile India) and Reskilll. Reskilll runs the platform, mentors, jury and on-site operations. VERIFIED
- **Tagline:** "Great ideas don't need desks" (#GreatIdeasDontNeedDesks). It is marketed as India's first phone-first, hybrid mobile hackathon.
- **Qualcomm:** no official partnership is named. One Qualcomm developer advocate (Kartikey Rawat) mentored in Bengaluru. VERIFIED
- **Contact:** sameera@reskilll.com. WhatsApp group linked from the portal.

## 2. Cities and dates (VERIFIED, portal)

| City | Event | Idea deadline | Venue |
|---|---|---|---|
| Bengaluru | 29 to 30 Aug | 25 Aug | WeWork Galaxy |
| Pune | 5 to 6 Sep | 1 Sep | WeWork Eon, Kharadi |
| Chennai | 12 to 13 Sep | 8 Sep | The Hive, OMR |
| **Hyderabad** | **26 to 27 Sep** | **22 Sep, 23:59:59 IST** | TBA |
| Grand Finale | 9 to 11 Oct, Bengaluru, 48 h | 5 Oct | TBA |

- **Pilots in June 2026:** Delhi NCR (8 hour sprint, 6 Jun, Gurugram, 80 builders) and Bengaluru (13 to 14 Jun, Scaler School of Technology, 30 h). Their rubric weighted Office Kit at 25%, phone-first at 25%, AI-native at 20%, problem fit at 20% and pitch at 10%.

## 3. How selection works (VERIFIED, portal)
1. Register.
2. Form a team (solo or 1 to 3; students and professionals cannot mix).
3. Pick a problem statement. The list is behind login.
4. The team leader submits the Phase 1 idea.
5. Screening produces a shortlist.
6. The shortlisted teams play the 30 hour on-site battle.

**Screening criteria (FAQ):** "novelty, tech impact, problem choice, idea/scope fit for 30 hours, and phone-first fit."

**Competition level (UNVERIFIED):**
- Bengaluru: about 10,000 ideas submitted and 27 professional teams shortlisted.
- Another participant claims "12,000+ applications, 120 selected".
- A Chennai applicant with a working APK plus a GitHub repo was not selected, and their repo had zero views.

## 4. Phase 1 submission form (VERIFIED, portal code)

| Field | Type and limit |
|---|---|
| Idea title | 5 to 200 characters ("A short, punchy name") |
| Description | 50 to 2000 characters ("What are you building, for whom, and how?") |
| Video walkthrough URL | optional |
| Prototype URL | optional ("live demo / repo") |
| Deck / document | **required**, ONE PDF or PPT, max 25 MB, or a link |
| Android proficiency | dropdown: None / Basic / Intermediate / Expert, shipped apps |
| LLM proficiency | dropdown: None / Cloud APIs only / Experimented with local LLMs / Deployed local LLMs on-device |
| Prior builds and hackathons | 1000 characters max ("Helps shortlisting") |
| What makes you stand out | 1000 characters max ("Skills, domain edge, prior collab, why this problem") |
| Checkbox | "original work and any pre-existing components are disclosed" |

## 5. On-site format (VERIFIED, portal)
- **Saturday:**
  - 08:00 check-in, with ID, NDA and the loaner phone.
  - 10:00 keynote and HackTracker/Office Kit teach-in.
  - 11:00 hacking starts.
  - 15:30 mentor round.
  - 19:00 to 22:00 Evaluation Round 1 (scored, no elimination).
- **Sunday:**
  - 09:00 to 12:00 Evaluation Round 2: "All teams judged at tables. Demo on the iQOO phone." R1 and R2 give the Top 10 per bucket.
  - 13:45 pitches: 3 to 5 minutes, live, demo on hardware.
  - 16:15 awards, including a "Most iQOO Usage" award. The top 3 students and top 3 professionals go to the Finale.
- **Red Light (about 55%):** "Laptops closed, all build routes through the phone" (phone only, via Office Kit).
- **Green Light (about 45%):** phone and laptop both allowed.
- **Rules:**
  - "The iQOO device is the build surface and the demo surface: every entry must run and pitch on the phone."
  - "A local or open-source model at the core earns brownie points, with the phone in the loop via Office Kit."
  - FAQ: "On-device inference targets the Snapdragon NPU (Sarvam, Gemma, Phi class models). **Office Kit bridges to the laptop when you need deeper compute.**"
  - Stacks: native Android, Flutter, React Native or PWA.
  - "Code written during the event window. No shipping a pre-built product." Pre-event drafting of ideas is allowed. Open-source libraries are allowed with attribution.
  - IP stays with the team.

## 6. Judging rubric at the event (VERIFIED, portal)

| Criterion | Weight | Scored by |
|---|---|---|
| End product quality: "Does it work, is it useful, would someone keep using it" | 30% | Jury |
| Novelty and impact | 20% | Jury |
| Creative phone use: "Camera, voice, on-device AI in the build" | 15% | HackTracker device data |
| Technical depth: "Architecture, code quality, robustness, real use of the hardware" | 15% | Jury |
| Office Kit usage: "Phone and laptop bridge use" | 10% | HackTracker device data |
| Demo: 3 to 5 minute pitch | 10% | Jury |

- 25% of the score is automatic telemetry.
- HackTracker is pre-installed. It logs "counts and durations only".

## 7. Office Kit and the device (VERIFIED)
- **Office Kit:** vivo's PC app (pc.vivoglobal.com), for Windows 10+ or macOS 10.14.6+, paired with an iQOO phone on OriginOS 6.
  - Features: screen mirroring, shared clipboard, file transfer with no size limit, and laptop keyboard and trackpad controlling the phone.
  - **It is a user-facing bridge, not a developer compute API.**
- **Loaner phone:** the iQOO 15, one per person (iQOO Instagram and press). It comes with HackTracker installed and Office Kit already paired.
  - Snapdragon 8 Elite Gen 5, OriginOS 6 on Android 16.
  - 12 or 16 GB RAM. **Which variant is loaned is unknown.**
  - 7000 mAh battery, 100 W charging, triple 50 MP cameras.

## 8. Tracks (VERIFIED)
- **City battles:**
  - FinTech and Commerce
  - Smart Education
  - HealthTech
  - Productivity
  - Smart Living
  - **Developer Tools** ("tools that help developers create, test, deploy, or collaborate faster using AI")
  - **Open Innovation** ("any domain, with a local or open source model at the core")
- **Finale tracks:** FinTech, Education and Health are dropped. Mobility and Community App are added.
- **Hyderabad blurb:** "Enterprise scale and health-data depth, from the campuses that staff them."

## 9. Prizes (VERIFIED, portal)
- ₹40 lakh total. ₹6 lakh per city.
- **Professionals:** ₹1.5L winner / ₹1L / ₹70K.
- **Students:** ₹1.5L / ₹80K / ₹50K.
- **Other:** special honours and a Most iQOO Usage award. The Finale amount is disputed (₹16L or ₹25L).

## 10. Jury and mentors so far
- **Bengaluru jury:** AWS India, nasscom, Avashya CTO, ClickHouse, Microsoft. Mentors from Kuku FM, Walmart, AmEx, PhonePe, Qualcomm and others.
- **Pune jury:** Swarovski, EPAM, Rover AI.
- **Chennai jury:** Mahindra, Aracor AI (Lesly Arun Franco, CTO), Scentric Networks.
- **Hyderabad:** not announced.
- **Pattern:** senior engineers and product and AI leads from industry. They will probe feasibility.

## 11. Previous winners (project details are mostly unpublished)
- **Pune (VERIFIED):**
  - 1st Team Red String (AIT Pune, student).
  - 2nd Team Kensai.
  - 3rd Team Chanakya.
  - Professional 1st runner-up: Merge Conflicts, an AI tree-health monitor that uses the phone camera and reports to PMC.
- **Chennai (partly verified):**
  - Students: Atreides, Apple, Leadmillers.
  - Professionals: runners-up One Man and Dhaneshvar.
  - One participant built an on-device JARVIS-like companion on the iQOO 15 (placement unknown).
- **Bengaluru (UNVERIFIED):**
  - Students: Chole Bhature (winner), Nexus.
  - Professionals: TOKITO (winner).
- **Observed pattern:** practical, phone-camera and on-device AI builds with a clear real-world user.
- **Reskilll's own advice:** "A well-built simple product beats a broken complex one." It suggests Phi-3, Gemma 2B and Whisper Tiny.

## 12. Unknowns (do not claim)
- Hyderabad venue, jury and mentors.
- The problem statement list (check it after logging in).
- The loaner RAM variant.
- The Finale prize.
- Any formal Qualcomm partnership.

## Key sources
- **Portal:**
  - https://iqoo.reskilll.com
  - https://iqoo.reskilll.com/guide
  - /terms
  - iqoo-delhi.reskilll.com and iqoo-blr.reskilll.com
- **Reskilll blogs:**
  - reskilll.com/blogs/iqoo-hackathon-2026-india-phone-first-ai-hackathon-iqoo-reskilll/
  - reskilll.com/blogs/iqoo-city-battles-hyderabad-on-device-ai-hackathon-sept-2026/
- **Press:**
  - FoneArena 489783
  - ITVoice
  - DigitalTerminal
- **iQOO:**
  - community.iqoo.com threads 167067 and 168552
  - https://www.iqoo.com/in/products/param/iqoo15
- **Social:**
  - X @IqooInd, @Reskilll, @TechSAM009, @ait_pune
  - Instagram @iqooind
  - Reddit r/ambitionarena7 1vzyplj, r/tamilyapping 1wbq6ei, r/androiddev 1werrva
