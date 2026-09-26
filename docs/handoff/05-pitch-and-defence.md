# The pitch, and how to defend it

## The story, in the order it should be told

**Who it is for.** Most people in India own a good phone and a weak computer, or no computer at all:
about 33 million phones above ₹30,000 shipped in 2025 against 7.3 million consumer PCs, and only
about 9% of households own a computer. Their best AI hardware is already in their hand.

**The problem.** New open models need roughly 12 GB. A budget laptop has about 4 GB free. A phone
has 2 to 10 GB depending on the device. Neither can load the model. A GPU upgrade costs more than
the laptop, and much professional code may not be sent to a cloud service anyway.

**What we do.** Pool the free memory of the devices you already own, over a link you control, and
expose it as one local address that existing tools already know how to talk to.

**What makes it a product rather than a script.** It decides. It measures each device, chooses
whether to run on one device, split the layers, or refuse, and it says why in a sentence a person
can read. It will tell you your phone is not worth using, and that is the feature.

**The proof.** Numbers measured on our own hardware, not a spec sheet:
- A mid-range realme runs a model at **20.3 tokens per second**, against 23 to 25 on the laptop, and
  reads prompts **faster** than the laptop.
- A split across laptop and phone runs at **8 to 12 tokens per second** over a link measured at
  0.77 ms, and the phone genuinely holds 500 MB of model.
- Splitting costs about **15% before any network exists**, so we only split when a model fits
  nowhere else.

## Claims that hold

- Models that fit on neither device can run across both.
- The phone is often the stronger AI device, because generation is limited by memory bandwidth and
  phone memory is faster than budget laptop memory.
- Only a few kilobytes cross the link per token, so latency matters far more than bandwidth.
- Nothing leaves the user's devices.
- The system refuses to make things worse, with a readable reason.

## Claims to never make

- Faster than a single device that can already hold the model. Every paper disagrees and so do our
  own measurements.
- Cloud speed or cloud quality.
- NPU acceleration. The upstream Hexagon backend is experimental and produces garbled output on this
  chip family.
- Works on any network. Venue Wi-Fi measured 65 to 154 ms and our own planner refuses it.
- That activations crossing a network are private. Research inverts them back into text.

## The hard questions, and the answers

**"Cloud inference is faster and cheaper. Why would anyone use this?"**
Concede immediately, then move the ground: permission, data gravity, availability, marginal cost.
Much professional code may not leave the building at all; Samsung banned staff from ChatGPT in 2023
after engineers pasted source code into it. Real work is not a small prompt, it is a repository or a
year of logs, and uploading that to save seconds is the wrong trade. From India a cloud round trip
is 108 to 158 ms before the model thinks, and median 5G upload is under 16 Mbps. And a device you
own costs nothing extra on the ten-thousandth job. **Then give the boundary honestly:** if your data
is not sensitive, you have internet, and you want the best possible answer, use the cloud.

**"A company like Samsung would just buy GPU laptops."**
Correct, and they are not our user. That story only proves the policy constraint is real. Our user
is the person for whom hardware is the constraint: students, freelancers, two-person teams, and
people at firms that forbid cloud AI but will not buy new machines either.

**"If I have a GPU laptop, is this useful?"**
No, and our own planner would tell you so. It would run the model on your laptop and refuse to
involve the phone.

**"How do I know the phone is actually doing the work?"**
Show the phone's memory: the helper process holds hundreds of megabytes of model. Show the layer map
in the panel. Kill the link and watch the answer stop.

**"Is a 20B local model not much worse than a frontier model?"**
Yes, for hard reasoning. For review, tests, documentation and log analysis, a good open model is
enough. Say this before they do.

**"Why is it slower when the phone joins?"**
Because splitting buys memory, not speed. Every token crosses the link once each way. We split only
when a model fits nowhere else, and we say so on the screen.

**"What about security? The docs say llama.cpp's RPC is insecure."**
They do, which is why it only ever runs on a private link, bound to that link's address, after
pairing with a per-device secret compared in constant time. Encryption of the control channel is
designed and not yet built.

## Positioning against the iQOO audience

Do not claim the product is iQOO-specific; they will know it is not. The honest and more flattering
version: it runs on any capable Android phone, and it runs **best** on theirs, because a Snapdragon
8 Elite Gen 5 with 12 or 16 GB has more usable memory and faster memory than almost anything else in
a pocket. Their own engineers can verify that, which is the only kind of compliment worth paying.

If a number helps: iQOO reached **4.3% of the Indian market in the first half of 2025, up from 2.7%,
growing 68%**, which works out at roughly three million phones in six months. Say "derived from
IDC's share figure", because deriving it is honest and inventing it is not.
