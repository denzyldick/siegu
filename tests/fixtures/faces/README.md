# E2E face-grouping fixtures

Public-domain portrait photographs of Albert Einstein, sourced from
[Wikimedia Commons](https://commons.wikimedia.org/wiki/Category:Albert_Einstein).
All four images are the same person and are used by the CI end-to-end test
(`scripts/e2e-face-grouping.sh`) to verify that face detection + grouping
places every photo into a single person group.

- `einstein_1.jpg` — Albert Einstein Head (cleaned)
- `einstein_2.jpg` — Albert Einstein sticks his tongue out
- `einstein_3.jpg` — Albert Einstein in later years
- `einstein_4.png` — Albert Einstein (1921 Nobel portrait)

Each image is the work of its respective photographer and is in the public
domain. Resized to ≤960px for repository size.
