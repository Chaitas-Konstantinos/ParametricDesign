---
name: from-photos
description: Turns photos, sketches and rough descriptions into concrete dimensions for a ParametricDesign model - finding scale from reference objects, choosing what to measure, adding fit allowances, and exposing every estimate as an adjustable param so the user can correct it. Use whenever the request includes photos, drawings or approximate sizes.
---

# From photos and descriptions to dimensions

Users describe what they want in words and photos, usually without a full set of measurements. Your job is to get to a design that is nearly right and **easy to correct with sliders**, without a long back-and-forth.

## 1. Collect every size you are given

Write down each explicit number with what it refers to ("the drawer is 42 cm wide inside", "cable about 6 mm"). Explicit numbers from the user win over anything you estimate.

## 2. Find the scale in a photo

Look for an object of known size in the same plane as the thing being measured:

| reference | size (mm) |
|---|---|
| credit / ID card | 85.6 × 54.0 × 0.76 |
| A4 paper | 210 × 297 |
| US letter paper | 216 × 279 |
| AA battery | ⌀ 14.5 × 50.5 |
| AAA battery | ⌀ 10.5 × 44.5 |
| 1 euro coin | ⌀ 23.25 |
| 2 euro coin | ⌀ 25.75 |
| US quarter | ⌀ 24.26 |
| US penny | ⌀ 19.05 |
| UK £1 coin | ⌀ 23.43 |
| standard pencil | ⌀ 7, 175-190 long |
| USB-A plug | 12.0 × 4.5 |
| USB-C plug | 8.3 × 2.5 |
| 3.5 mm audio plug | ⌀ 3.5 (housing ⌀ 6-10) |
| LEGO 2×4 brick | 31.8 × 15.8 × 9.6 |
| ruler / tape measure | read it |
| floor tiles / bricks | ask, or estimate 200-600 |

Measure the reference and the target along the same direction and close to the same depth from the camera. Perspective makes nearer things look bigger, so don't compare objects at very different distances. Prefer the largest dimension (relative error is smaller). If the photo shows a ruler or a tape measure, use it.

If there is no reference, use typical sizes (a mug ⌀ 80-90 × 95 tall; a phone 70-78 × 145-165 × 8-9; a laptop 300-360 × 210-250 × 15-20; a toothbrush handle ⌀ 12-18; a hand fits 85-100 wide) and make the uncertain sizes prominent params.

## 3. Decide what must fit and add allowances

- **Containers / holders**: inside size = measured object + allowance. Use 0.5-1 mm per side for rigid objects, more for soft, varying or grippy things (cables 1-2 mm, food or clothing 5 mm+).
- **Things that slide onto or into something** (clips on a tube, caps): measured size + 2 × clearance (0.2 mm).
- **Press fits** (a peg into a hole): 0.1 mm per side.
- **Mounting to a wall or furniture**: the screw size, the hole spacing of existing holes, and enough material behind the screw.

## 4. Turn estimates into params

Every dimension you **estimated** (not given exactly) becomes a param:

- a range around your estimate of about −30 % / +50 % (wider if you are unsure);
- a label that says what to measure: `"Phone width (measure across the case)"`, `"Shelf board thickness"`;
- a description saying how you estimated it: `"Estimated from the photo using the credit card for scale; measure and correct."`

Put the 1-3 sizes the user most likely needs to check **first** in `params`, in a `"Measure these"` group.

## 5. Say what you assumed, briefly

In `meta.description` and in your reply, list the estimated sizes and what to measure: "I estimated the mug handle opening at 22 × 60 mm from the photo. Check `handle_gap` before printing." One or two lines, not a questionnaire.

## When to ask instead

Ask one short question **only** if the design cannot work without it **and** nothing in the photos or text gives a scale. For example: a case for a specific device model you don't know the size of. Even then, give the design with your best estimate as params and ask the question in the same reply.

## Reading sketches

Hand sketches show shape and proportions, not sizes. Take numbers written on the sketch literally and keep the proportions for the rest. Read arrows and dimension lines as the distance between the features they touch. If the sketch has a front view and a side view, they share heights.
