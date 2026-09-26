# Form answers

## Idea title
MeshAI: run AI models your laptop can't, using the phone you already own

## Description
MeshAI pools the memory of a phone and a laptop into one private machine, so developers can run open AI models that neither device can load alone.

Who it is for: developers and students with a strong phone and a budget laptop. New open models like gpt-oss-20b need about 12 GB. An 8 GB laptop has around 4 GB free, and a 16 GB phone has around 10 GB usable, so neither can run it alone. A GPU upgrade often costs more than the laptop, and many companies do not allow code to be pasted into cloud AI. India sold about 33 million phones above ₹30,000 in 2025 and only 7.3 million consumer PCs, so the phone is often the strongest computer a developer owns.

How it works: the MeshAI app on the iQOO phone scans a QR code on the laptop and joins. It measures free RAM, CPU, GPU and NPU speed, temperature, battery and link latency. A scheduler then picks a plan. If the model fits on one device, it runs there. If not, its layers are split across the laptop and the phone using llama.cpp RPC, and only a few KB cross the link per token. A second phone can join for bigger models such as Qwen3-Coder-30B. The mesh appears as one local OpenAI compatible API, so coding tools work unchanged, and a job queue runs longer tasks like writing tests, reviewing a branch or reading logs, fully offline.

The phone is the core: it holds most of the model, runs its own inference, and shows a live dashboard of layers, tokens per second, heat and battery. Office Kit moves model files to the phone and mirrors the dashboard on the laptop.

We are honest about limits. Splitting does not beat one device that can already hold the model, so the scheduler only splits when it must. The 30 hour build targets one laptop and one iQOO 15 running a split model with the app, scheduler and API. The second phone and parallel jobs are stretch goals.

## Video walkthrough URL
Leave empty.

## Prototype URL
Leave empty. Add the repo link later if you publish one.

## Deck / document
Upload v2/MeshAI_Pitch_Deck_v3.pdf

## Android proficiency
Expert / shipped apps

## LLM proficiency
Cloud APIs only. Pick "Experimented with local LLMs" only if one of you has actually run a model locally (Ollama, LM Studio, llama.cpp).

## Prior builds & hackathons
We have built and shipped mobile and AI products end to end. These include AI voice calling systems that chain speech to text, an LLM and text to speech; Flutter and Android health apps with local first storage and an AI chat assistant; pose and fitness tracking using the phone camera; and full stack apps with auth, payments, maps, real time sync and admin dashboards. This work taught us to connect mobile UI, device APIs, backends and AI models, which is exactly what MeshAI needs. [Add any hackathon results or public repos here, only if true.]

## What makes you and your team stand out?
We are not building another chatbot on top of a cloud API. MeshAI works on the layer underneath: how a phone and a laptop can share one model. That needs Android depth (a native NDK worker, a foreground service, reading thermal and battery state, camera pairing) together with an understanding of how LLMs actually use memory, and our background covers both. We have done the homework: we checked llama.cpp's RPC and Adreno support, the known NPU issues on this chip, and published research on splitting models across edge devices, and we scoped the build to what fits in 30 hours. We picked this problem because we live it: our phones are stronger than our laptops, and we want to run good models on code we cannot send to the cloud.
