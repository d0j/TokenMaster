# Embedded pricing catalog

Catalog ID: `openai-api-2026-07-16-v1`

Retrieved and reviewed: 2026-07-16

The Rust catalog is a small curated OpenAI/Codex table. It is not a vendored LiteLLM
database and it is never refreshed at runtime. Rates are USD per one million tokens.
Cached-input values for models documented without a cache discount intentionally equal
the uncached-input rate.

Primary official sources:

- <https://developers.openai.com/api/docs/models>
- <https://developers.openai.com/api/docs/models/gpt-5.6-sol>
- <https://developers.openai.com/api/docs/models/gpt-5.6-terra>
- <https://developers.openai.com/api/docs/models/gpt-5.6-luna>
- <https://developers.openai.com/api/docs/models/gpt-5.5>
- <https://developers.openai.com/api/docs/models/gpt-5.5-pro>
- <https://developers.openai.com/api/docs/models/gpt-5.4>
- <https://developers.openai.com/api/docs/models/gpt-5.4-pro>
- <https://developers.openai.com/api/docs/models/gpt-5.4-mini>
- <https://developers.openai.com/api/docs/models/gpt-5.4-nano>
- <https://developers.openai.com/api/docs/models/gpt-5.3-codex>
- <https://developers.openai.com/api/docs/models/gpt-5.2>
- <https://developers.openai.com/api/docs/models/gpt-5.1>
- <https://developers.openai.com/api/docs/models/gpt-5>
- <https://developers.openai.com/api/docs/models/gpt-5-mini>
- <https://developers.openai.com/api/docs/models/gpt-5-nano>
- <https://openai.com/api-priority-processing/>

Behavioral cross-check only:

- ccusage commit `997ad7f90189867d9f218aa0e7401586e3b9fde8`
- its locked LiteLLM revision `49ca04d8c3ddea336237ce6f3082dbc26d19e944`

The catalog intentionally omits a model/tier/context combination when the reviewed
official source does not establish that combination. Missing rules are returned as
unavailable, never as zero and never as an inferred multiplier.
