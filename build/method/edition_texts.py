#!/usr/bin/env python3
"""Texts of the edition layer of PCP 1.0. One module so that every edition is produced from the same
source and differs only where the edition differs. Nothing here is copied into the Implementation packet."""

DATE = "13 November 2026"
ED = {
    "PERSONAL":   {"name": "Personal Edition",   "short": "Personal",   "folder": "PCP_1.0_Personal_Edition"},
    "COMMERCIAL": {"name": "Commercial Edition", "short": "Commercial", "folder": "PCP_1.0_Commercial_Edition"},
    "ENTERPRISE": {"name": "Enterprise Edition", "short": "Enterprise", "folder": "PCP_1.0_Enterprise_Edition"},
}

# --------------------------------------------------------------------------------------------------
EVIDENCE = """# Parrot Council Protocol (PCP) 1.0 | Evidence status

**Read this before relying on the product.** It applies to every edition, and to the method inside the product, Adversarial Claim Review (ACR).

As of %(date)s:

1. **Not peer reviewed.** No part of PCP or ACR has been submitted to or accepted by a peer-reviewed venue, and no standards body has reviewed or adopted it.
2. **Not independently reviewed.** Every review of these materials was performed by the party that drafted them or by AI systems working at the creator's direction. No independent expert, auditor or institution has examined them.
3. **Not empirically evaluated.** No controlled evaluation, pilot, field trial or other prospective study of ACR in use has been conducted. No assistant, model or host has been tested against ACR under conditions that support a conclusion. The behavioral probes supplied in the Implementation packet were run on 12 November 2026 in uncontrolled sessions only: by the party that drafted ACR, with assistants of one provider, most probes once or twice, with screens that are text patterns and replies read by that same party. Those runs show that the probes can be delivered and screened. They support no rate and no conclusion about effect, and they are not the evaluation this item says is missing.
4. **No observational evidence of effect.** There is no retrospective or post-deployment study, and no outcome data about what ACR's use changed. Two single uncontrolled instances have been examined, each under conditions that were not recorded. In one, an assistant produced a review in ACR's format that departed from ACR's rules: it attributed to its subject a claim the subject disclaims, reported execution it did not demonstrate, and used status codes ACR does not define. In the other, an assistant handed the whole Implementation packet with no request reported it as loaded and indexed and summarized it with a structural error. Two instances support no rate and no conclusion about effect. They show that ACR's format and vocabulary can be reproduced without its substance.
5. **What the release checks are.** The verifiers, the tool tests and the fault-injection runs verify the files and reference programs against their own specification. That is verification of the artifacts. It is not validation that ACR achieves its purpose.
6. **What the cited sources are.** The sources cited in each document support individual design choices in the settings those sources studied. None of them evaluates ACR, and together they do not validate it.

## What follows from this

- ACR's statements about what a participant must do are specifications of intended behavior. They are not findings that an assistant behaves that way.
- Every benefit that ACR is designed to produce is an untested hypothesis.
- ACR's text reaches an assistant's reasoning as well as its reply, and its effect there is unmeasured. An assistant's reply can differ from its reasoning, and a displayed reasoning trace can differ from what the assistant actually computed. ACR places its duties on the reply and cannot detect such a difference by itself. An assistant's statement that it has loaded or understood the product is a reply like any other and establishes neither. A probe is recognizable as a test, so a passed probe does not establish ordinary behavior.
- Nothing in this product may be described as validated, proven, certified, peer reviewed or established. The Core Specification, Section 1.1, forbids a participant from describing ACR in those terms.
- A person or organization that needs evidence has to produce it for its own setting. The Implementation packet supplies the means: behavioral probes with a three-arm comparison, Calibration (Core Specification, Section 14.4), which measures whether the method arrived in a given assistant as delivered, and the comparison with ordinary work described in Section 14.2.
- A license to use the product is not a statement by the Licensor that the product is fit for any decision of consequence.

This is the product's evidence statement. The Implementation packet carries the method's own statement (`RELEASE/Known_Limitations.md`), which says the same of the method and does not speak of the product's plugins. Other documents refer to one of the two and do not restate it.
"""

# --------------------------------------------------------------------------------------------------
PRICING = """# Parrot Council Protocol (PCP) 1.0 | Price schedule

**This schedule is fixed.** It is the price structure of the product, set by the rights holder, Kirtus Justus. It is not a proposal, an estimate, an introductory offer or a starting point for the Personal Edition.

| Edition | Price | Basis |
| --- | --- | --- |
| Personal | **US$5.00** | One payment for one license held by one natural person. Not a subscription: no renewal and no recurring charge. |
| Commercial | No published price | Licensed only under a written agreement negotiated individually with the rights holder. |
| Enterprise | No published price | Licensed only under a written agreement negotiated individually with the rights holder. |

## Rules

1. **Personal Edition.** The price is five United States dollars (US$5.00), paid once. It is the whole price of the Personal Edition license. It is not negotiable in either direction, and no installment, renewal, usage or maintenance charge is attached to it.
2. **Commercial and Enterprise Editions.** There is no list price, price range, per-seat rate, tier table or standard discount. The fee and the terms exist only in a written agreement signed by the rights holder for the individual licensee. A figure that is not in such an agreement is not a price.
3. **No crossing between editions.** The Personal price buys the Personal license only. It does not buy, discount or count toward a Commercial or Enterprise license, and a Personal license does not permit the uses that those editions license.
4. **What a price does not include.** No edition includes access to any AI model, API credit, compute or hosting. Those are bought separately from a provider the licensee chooses, under that provider's own terms and charges.
5. **Currency and tax.** The Personal price is stated in United States dollars. Any sales, goods-and-services or value-added tax that the law requires the seller to collect is not part of the US$5.00, and the total including that tax is shown before payment.
6. **Authority.** Only the rights holder sets or changes this schedule, and only by issuing a new schedule. A change does not alter a license already granted.

The terms of each edition's license are in `LICENSE.md` of that edition.
"""

# --------------------------------------------------------------------------------------------------
NOTICE = """# Parrot Council Protocol (PCP) 1.0 | Notice

**Proprietary material. All rights reserved.**

Copyright (c) 2026 Kirtus Justus. All rights reserved, to the fullest extent permitted by applicable law.

## 1. Ownership

Parrot Council Protocol (PCP) is a commercial product of Kirtus Justus ("the Licensor"). The method it contains is Adversarial Claim Review (ACR). Every file in this edition is the Licensor's proprietary material, and the Licensor keeps every right in it: the documents at this level, the Implementation packet (`ACR_1.0_Implementation.zip`) and the Readers packet (`PCP_1.0_Readers.zip`).

## 2. Supplied under license only

PCP is licensed, not sold. A licensee receives the permissions stated in `LICENSE.md` of this edition and no others: in particular, no right to redistribute the product, to distribute changed versions of it or to use it beyond that license. The Implementation packet is delivered as readable text and Python so that a licensee can inspect it and load it into tools. That is a manner of delivery, and it grants nothing further.

## 3. No rights without a license

Holding a copy grants nothing. The only permissions are those in `LICENSE.md` of this edition, and they belong only to the licensee that document identifies. All other rights are reserved.

## 4. Names and emblem

"Parrot Council Protocol", "PCP" and the bird emblem identify this product, and "Adversarial Claim Review" and "ACR" identify its method. The Licensor uses them as its brand. No trademark registration is claimed. Anyone may use the names to refer truthfully to the product or the method. Nobody may use them, or the emblem, to name or promote other goods or services, or in a way that suggests the Licensor's endorsement.

## 5. Origin of the texts

The creator is Kirtus Justus. The texts were drafted with AI systems working at the creator's direction, as the header of each document states: Claude (Anthropic) and, for the Statistics and Quality Assurance Specifications, ChatGPT (OpenAI). Neither company has reviewed, endorsed or certified PCP. Their names appear only to state that origin.

## 6. Cited bodies

Standards bodies, publishers, products and authors are named in the documents only to identify the sources cited. None of them has reviewed, endorsed or certified PCP or ACR.

## 7. Third-party components

The PDF reading copies embed subsets of two typefaces: Inter (Copyright 2016 The Inter Project Authors; SIL Open Font License, Version 1.1) and DejaVu Sans Mono (Copyright 2003 Bitstream, Inc.; Bitstream Vera license, with DejaVu changes in the public domain). Those licenses permit embedding in documents and do not change the license of the documents. The programs in the Implementation packet use only the Python standard library. No other third-party code, text or artwork is included.

## 8. This notice stays with the product

This notice, and the notice inside the Implementation packet, stay with every copy of the product and of any part of it.
"""

# --------------------------------------------------------------------------------------------------
LICENSE_PERSONAL = """# Parrot Council Protocol (PCP) 1.0 | Personal Edition License

**Licensor:** Kirtus Justus, the rights holder.  
**The Materials:** PCP Version 1.0, Personal Edition: the ACR 1.0 Implementation packet, the PCP 1.0 Readers packet and the documents supplied with them in this edition.  
**License fee:** US$5.00, paid once (`PRICING.md`).

The Materials are proprietary. They are licensed, not sold, and the Licensor keeps every right that this document does not grant. This document states everything you are permitted to do with them. Read it together with `NOTICE.md` and `EVIDENCE_STATUS.md`.

## 1. Who is licensed

1.1 This license is granted to one natural person ("you"): the person who acquired the Personal Edition from the Licensor or from a seller the Licensor has authorized.

1.2 It takes effect when that acquisition is complete. It has no expiry date and continues until it ends under section 9.

1.3 A copy obtained in any other way carries no license.

1.4 An organization cannot hold this license, and it cannot be shared, transferred or resold.

## 2. What you may do

For your own personal, study and research purposes you may:

(a) keep and read the Materials, and make the copies you reasonably need for your own use and backup, on devices you control;

(b) run the programs in the Implementation packet;

(c) apply ACR to your own work, and load the Materials or parts of them into AI tools and services that you use for yourself, including services operated by others, as section 4 provides;

(d) adapt the Materials for your own private use, keeping every notice in place.

Nothing in this license limits a use that the law allows without the rights holder's permission.

## 3. What you may not do

You may not:

(a) give, sell, rent, lend, sublicense, publish, post or otherwise make the Materials, an adaptation of them or any substantial part of either available to another person;

(b) use the Materials in the course of a business or paid professional service, or for an employer, client or organization. Those uses require the Commercial or Enterprise Edition;

(c) remove or alter a notice of ownership, origin or evidence status;

(d) present the Materials, or an adaptation of them, as your own work or under another name;

(e) use the Materials to train or fine-tune a machine-learning model, or to build a product or service supplied to others;

(f) state or imply that the Materials, or work done with them, are validated, certified, peer reviewed or endorsed by the Licensor or by anyone else;

(g) use the names "Parrot Council Protocol" or "PCP", or the emblem, to identify goods or services of your own.

## 4. AI tools and services

When you load the Materials into a service operated by someone else, that service's own terms govern what it does with them. Where the service lets you choose, you must choose settings that keep the Materials private and exclude them from model training. The Personal Edition supplies no AI model, API access, compute or hosting.

## 5. Your work is yours

Reviews, findings, records and other results that you produce by applying ACR are yours. The Licensor claims no right in them. You may share them freely, and you may say truthfully that you used Adversarial Claim Review (ACR) 1.0 to produce them.

## 6. Ownership

The Licensor keeps all rights in the Materials that this license does not expressly grant. The conditions in this license are terms of your access to the Materials. They apply whether or not a particular element of the Materials is protected by copyright in a particular country.

## 7. Evidence status and reliance

7.1 The Materials have not been peer reviewed, independently reviewed or empirically evaluated, as `EVIDENCE_STATUS.md` states. You accept them on that understanding.

7.2 The Materials are not legal, medical, financial, engineering, security or other professional advice, and they are not a safety certification. They grant no permission to act in any system. Decisions you make using them are yours.

## 8. Warranty and liability

8.1 Rights that consumer protection law gives you, and that cannot lawfully be excluded or limited by agreement, are not affected by anything in this license.

8.2 Subject to 8.1, the Materials are provided as they are. The Licensor does not promise that they are accurate, complete, free of error or fit for a particular purpose.

8.3 Subject to 8.1, and to the extent the law permits, the Licensor's total liability arising from this license is limited to the license fee you paid, and the Licensor is not liable for indirect or consequential loss, loss of data or loss of profit.

8.4 Nothing in this license prevents you from publishing an honest review of the product, from joining a class proceeding or from bringing a claim before a court or tribunal that has authority over it.

## 9. Ending

9.1 You may end this license at any time by deleting every copy of the Materials you hold.

9.2 The Licensor may end this license by notice to you if you seriously breach section 3. When the license ends you must stop using the Materials and delete every copy. Section 5 continues.

9.3 Any right to cancel the purchase or to receive a refund is set by the terms of sale shown to you when you bought, and by the consumer protection law that applies to you. This license does not reduce those rights.

## 10. General

10.1 This license and the terms of sale shown to you at purchase are the whole agreement about the Materials. The Licensor cannot change the terms of a license you already hold without your agreement.

10.2 If a court finds part of this license unenforceable, the rest remains in force.

10.3 This license is governed by the laws of British Columbia and the federal laws of Canada that apply there. That choice does not deprive you of the protection of mandatory consumer protection law of the place where you live.
"""

_ORG_COMMON_HEAD = """# Parrot Council Protocol (PCP) 1.0 | %(name)s License Conditions

**Licensor:** Kirtus Justus, the rights holder.  
**The Materials:** PCP Version 1.0, %(name)s: the ACR 1.0 Implementation packet, the PCP 1.0 Readers packet and the documents supplied with them in this edition.  
**License fee:** set by individually negotiated written agreement; no price is published (`PRICING.md`).

The Materials are proprietary. They are licensed, not sold, and the Licensor keeps every right that the Agreement and these Conditions do not grant. Read this document together with `NOTICE.md` and `EVIDENCE_STATUS.md`.

## 1. How this edition is licensed

1.1 The %(name)s is licensed only under a written %(short)s License Agreement signed by the Licensor and the licensed organization ("the Agreement"). These Conditions form part of every such Agreement. Where the Agreement and these Conditions differ, the Agreement prevails.

1.2 **Without a signed Agreement there is no license.** Possession of this edition, payment for another edition, or a quotation, purchase order or correspondence that the Licensor has not signed as an Agreement grants no right to use the Materials.

1.3 The Agreement, not this document, sets: the licensed organization ("the Licensee"); the fee and how it is paid; the term and any renewal; the users, sites or affiliates covered; any support, maintenance or later versions; and the parties' liability to each other.
"""

LICENSE_COMMERCIAL = _ORG_COMMON_HEAD + """
## 2. Standard scope

Unless the Agreement says otherwise, the Licensee may, in its own work:

(a) let its personnel (its employees, and individual contractors while working for it) keep, read and use the Materials, and make the copies reasonably needed for that use;

(b) run the programs in the Implementation packet;

(c) apply ACR to the Licensee's own work, including work it performs for its clients;

(d) have its personnel load the Materials or parts of them into the AI tools and services they use for the Licensee's work, as section 4 provides;

(e) adapt the Materials for internal use, for example by mapping ACR's duty names to the Licensee's own role names, keeping every notice in place.

## 3. Outside the standard scope

The Licensee may not, unless the Agreement expressly allows it:

(a) give, sell, rent, lend, sublicense, publish or otherwise make the Materials, an adaptation of them or any substantial part of either available outside the Licensee, including to clients and affiliates;

(b) build the Materials into software, an automated pipeline or a hosted platform operated for groups of users. That use is licensed by the Enterprise Edition;

(c) offer the Materials, or a product or service that contains them, to third parties;

(d) remove or alter a notice of ownership, origin or evidence status;

(e) use the Materials to train or fine-tune a machine-learning model;

(f) state or imply that the Materials, or work done with them, are validated, certified, peer reviewed or endorsed by the Licensor or by anyone else;

(g) use the names "Parrot Council Protocol" or "PCP", or the emblem, to identify goods or services of its own.

## 4. AI tools and services

The Commercial Edition supplies no AI model, API access, compute or hosting. Where the Licensee's personnel load the Materials into a service operated by someone else, the Licensee must use that service under terms and settings that keep the Materials private to the Licensee and exclude them from model training.

## 5. The Licensee's work is its own

Reviews, findings, records and other results produced by applying ACR belong to the Licensee, and it may deliver them to its clients. The Licensor claims no right in them. The Licensee may state truthfully that Adversarial Claim Review (ACR) 1.0 was used. It must not present that use as an assurance, a certification or evidence of quality, and it must pass on the substance of `EVIDENCE_STATUS.md` to a client who is told that ACR was used and who relies on that fact.

## 6. Ownership

The Licensor keeps all rights in the Materials that the Agreement and these Conditions do not expressly grant. These Conditions apply whether or not a particular element of the Materials is protected by copyright in a particular country.

## 7. Evidence status and reliance

7.1 The Materials have not been peer reviewed, independently reviewed or empirically evaluated, as `EVIDENCE_STATUS.md` states. The Licensee accepts them on that understanding.

7.2 The Materials are not professional advice and not a safety or compliance certification. They grant no permission to act in any system. The Licensee is responsible for deciding whether and how to use them, for testing them in its own setting and for the decisions it makes.

## 8. Warranty and liability

Except as the Agreement expressly states, and to the extent the law permits, the Materials are provided as they are, without any promise that they are accurate, complete, free of error or fit for a particular purpose. The parties' liability to each other is as the Agreement states.

## 9. Ending

When the Agreement ends, the Licensee must stop using the Materials and delete or return every copy, except copies that the law requires it to keep. Section 5 continues.
"""

LICENSE_ENTERPRISE = _ORG_COMMON_HEAD + """
## 2. Standard scope

Unless the Agreement says otherwise, the Licensee may, in its own work:

(a) let its personnel (its employees, and individual contractors while working for it) keep, read and use the Materials, and make the copies reasonably needed for that use;

(b) run the programs in the Implementation packet;

(c) apply ACR to the Licensee's own work, including work it performs for its clients;

(d) have its personnel load the Materials or parts of them into the AI tools and services they use for the Licensee's work, as section 5 provides;

(e) adapt the Materials for internal use, keeping every notice in place.

## 3. Enterprise rights

The following rights are what distinguish the Enterprise Edition. Each exists only to the extent the Agreement grants it, for the entities and systems the Agreement names:

(a) **Integration.** Building the Implementation packet, in whole or in part, into the Licensee's internal software, agent platforms and automated pipelines.

(b) **Internal hosting.** Operating an internal service that applies ACR for the Licensee's personnel.

(c) **Internal distribution.** Distributing the Materials to the affiliates the Agreement names, with every notice in place.

(d) **Services.** Support, maintenance, later versions, indemnity and cooperation with the Licensee's audits, as the Agreement describes them. None is implied.

## 4. Outside the license

The Licensee may not, unless the Agreement expressly allows it:

(a) give, sell, rent, lend, sublicense, publish or otherwise make the Materials, an adaptation of them or any substantial part of either available outside the Licensee and the affiliates the Agreement names;

(b) offer the Materials, or a product or service that contains them, to customers or other third parties;

(c) remove or alter a notice of ownership, origin or evidence status. In an integration, the notice inside the Implementation packet stays with the integrated files, and the integration records the Implementation packet's release fingerprint;

(d) use the Materials to train or fine-tune a machine-learning model;

(e) state or imply that the Materials, a system that integrates them or work done with them are validated, certified, peer reviewed or endorsed by the Licensor or by anyone else;

(f) use the names "Parrot Council Protocol" or "PCP", or the emblem, to identify goods or services of its own.

## 5. AI models, hosts and services

The Enterprise Edition supplies no AI model, API access, compute or hosting. Where the Materials are loaded into, or integrated with, a model or service operated by someone else, the Licensee must do so under terms and settings that keep the Materials private to the Licensee and exclude them from model training.

## 6. The Licensee's work is its own

Reviews, findings, records and other results produced by applying ACR belong to the Licensee, and it may deliver them to its clients. The Licensor claims no right in them. The Licensee may state truthfully that Adversarial Claim Review (ACR) 1.0 was used. It must not present that use, or a statement that a host supports ACR, as an assurance, a certification or evidence of quality, and it must pass on the substance of `EVIDENCE_STATUS.md` to a client who is told that ACR was used and who relies on that fact.

## 7. Ownership

The Licensor keeps all rights in the Materials that the Agreement and these Conditions do not expressly grant. These Conditions apply whether or not a particular element of the Materials is protected by copyright in a particular country.

## 8. Evidence status and reliance

8.1 The Materials have not been peer reviewed, independently reviewed or empirically evaluated, as `EVIDENCE_STATUS.md` states. The Licensee accepts them on that understanding.

8.2 The Materials are not professional advice and not a safety or compliance certification. They grant no permission to act in any system, and a control described in them is enforced only where the Licensee's own systems enforce it. The Licensee is responsible for deciding whether and how to use them, for testing them in its own setting and for the decisions it makes.

## 9. Warranty and liability

Except as the Agreement expressly states, and to the extent the law permits, the Materials are provided as they are, without any promise that they are accurate, complete, free of error or fit for a particular purpose. The parties' liability to each other, and any indemnity, are as the Agreement states.

## 10. Ending

When the Agreement ends, the Licensee must stop using the Materials, remove them from the systems that integrate them and delete or return every copy, except copies that the law requires it to keep. Section 6 continues.
"""

LICENSE = {"PERSONAL": LICENSE_PERSONAL,
           "COMMERCIAL": LICENSE_COMMERCIAL % {"name": "Commercial Edition", "short": "Commercial"},
           "ENTERPRISE": LICENSE_ENTERPRISE % {"name": "Enterprise Edition", "short": "Enterprise"}}

# --------------------------------------------------------------------------------------------------
GUIDE_HEAD = """# PCP 1.0 | %(name)s Guide

**INFORMATIVE.** This guide explains how the %(name)s is meant to be used. It adds no requirement to ACR and no term to the license. The Core Specification governs the method, `LICENSE.md` governs what is permitted, and `EVIDENCE_STATUS.md` states what has and has not been established about the product.
"""

GUIDE = {}
GUIDE["PERSONAL"] = GUIDE_HEAD % {"name": "Personal Edition"} + """
## What you hold

- `ACR_1.0_Implementation.zip`: the method. Sources, the kernel, tools, probes and release records. It is named ACR throughout and is the part an AI assistant is given as text.
- `PCP_1.0_Readers.zip`: typeset reading copies of every document, for you to read, and the brand identity.
- `PCP_1.0_Plugins/`, `PCP_1.0_Machine/` and `PCP_1.0_OS/`: the two plugins for Claude, the release as data with the design profiles, and the PCP OS documents. `README.md` describes each. With Claude Code, a plugin is the shorter path than this guide.
- `LICENSE.md`, `NOTICE.md`, `PRICING.md`, `EVIDENCE_STATUS.md` and this guide.

The Implementation packet is the same in every edition, byte for byte. The method does not vary with the license.

## The shortest useful path

1. Read `EVIDENCE_STATUS.md` and `LICENSE.md`.
2. In the Readers packet, open `START_HERE.pdf`, then `MODULES/01_Core/Overview.pdf`.
3. Extract the Implementation packet. Give your assistant `KERNEL_CARD.md` where there is a small character limit, or `KERNEL.md` otherwise. Do not load the whole packet into a conversation. An assistant that is handed it will usually answer with a summary and call the packet loaded; that reply does not put ACR in use and does not show what was read (Safety Handbook, question 33).
4. Ask for ACR by name when you want a claim, a change or a piece of work reviewed. At all other times, work normally. Say "stop" to end ACR use; it stops at once (Core Section 15.2).
5. If you intend to ask for the method as PCP, add the one-sentence alias given in the Readers packet's `README.md` to your assistant's instructions. Without it, the name has no ACR meaning to the assistant, which reads it as ordinary text and asks only if that would change what it does next (Core Section 15.6).

## What this configuration can and cannot tell you

Giving an assistant the card or the kernel as text is the Lightweight configuration (Integration Guide, section 3). Nothing is enforced in it: the assistant follows the text or it does not. With no tools and no sources, a finding about an external fact will be conditional or unresolved, and it should come with a labelled best estimate (Core Sections 5.2 and 15.8). Treat a confident finding of "supported" from recall alone as a failure of the assistant, not as a result. If your assistant shows its reasoning, read it as something it wrote, not as a record of what it did: a doubt that appears there and not in the reply is a defect in the reply, as is a step that the reply reports as done and the reasoning only planned, and a tidy trace proves nothing (Core Sections 4.4 and 15.8; Safety Handbook, question 34).

## Going further

The Standard configuration adds exact clause lookup (`TOOLS/acr.py`), the release verifier and the behavioral probes. The probes are the only way to learn whether your assistant follows the operating floor. Run them in three arms: no ACR text, the card and the kernel (Integration Guide, section 8). A probe that passes without ACR shows that the assistant already behaved that way.

## What the license covers

Your own personal, study and research use, by you alone. Results you produce are yours. Use for an employer, a client or a business needs the Commercial or Enterprise Edition (`LICENSE.md`, sections 2 and 3).
"""

GUIDE["COMMERCIAL"] = GUIDE_HEAD % {"name": "Commercial Edition"} + """
## What the organization holds

- `ACR_1.0_Implementation.zip`: the method. Sources, the kernel, tools, probes and release records. It is named ACR throughout and is the part an AI assistant is given as text.
- `PCP_1.0_Readers.zip`: typeset reading copies of every document, for people, and the brand identity.
- `PCP_1.0_Plugins/`, `PCP_1.0_Machine/` and `PCP_1.0_OS/`: the two plugins for Claude, the release as data with the design profiles, and the PCP OS documents. `README.md` describes each. With Claude Code, a plugin is the shorter path than this guide.
- `LICENSE.md` (the License Conditions that form part of the signed Agreement), `NOTICE.md`, `PRICING.md`, `EVIDENCE_STATUS.md` and this guide.

The Implementation packet is the same in every edition, byte for byte. The method does not vary with the license, so a finding made here rests on the same rules as a finding made anywhere else.

## Adopting ACR in a team

1. **Decide the form of adoption.** ACR can be a standing policy for named kinds of work, or it can be commissioned review by review. Receiving the packet adopts nothing (Core Section 2.1). Ordinary work stays the default (Core Section 1.2).
2. **Name who directs.** Each use has a principal, and a commissioned review has a Director. Direction decides what is done; it cannot make a finding say more than its evidence supports (Core Sections 3.2, 3.4 and 15.1).
3. **Use the team's own role names.** ACR's duty names are labels for functions, not job titles and not characters. Keep the canonical names only in consequential records (Core Sections 3.1 and 15.7).
4. **Keep records where the team already keeps them.** ACR prescribes no database and no extra forms. What matters is that findings and controlling conditions are committed outside the assistant's context (Core Sections 1.3 and 8.5).
5. **Use the Standard configuration.** Give assistants the kernel, with clause lookup available, and run the release verifier once (Integration Guide, section 3).
6. **Test before relying.** Run the behavioral probes on each assistant the team actually uses, in three arms, and have a person read the transcripts (Integration Guide, section 8). Where the assistant returns a reasoning trace, read it against the reply (`TEMPLATES/Trace_Reading.md` lists what to look for), and compare reasoning tokens across the arms. Repeat when the assistant or its version changes. A probe is recognizable as a test; a pass sets no floor for ordinary work.
7. **Declare the alias if people will say PCP.** One sentence in the assistant's instructions; the Readers packet's `README.md` gives it.

## Work for clients

Findings and reports produced with ACR are the organization's and may be delivered. The Materials may not. A statement that ACR was used should identify the exact version and say what was actually performed (Core Section 14.2). It is not an assurance, and a client who relies on it should be told the evidence status (`LICENSE.md`, section 5).

## What this edition does not license

Building ACR into software, automated pipelines or hosted platforms for groups of users, and distribution to affiliates, are Enterprise uses (`LICENSE.md`, section 3).
"""

GUIDE["ENTERPRISE"] = GUIDE_HEAD % {"name": "Enterprise Edition"} + """
## What the organization holds

- `ACR_1.0_Implementation.zip`: the method. Sources, the kernel, tools, probes and release records. It is named ACR throughout and is the only part that is integrated or given to an AI assistant.
- `PCP_1.0_Readers.zip`: typeset reading copies of every document, for people, and the brand identity.
- `PCP_1.0_Plugins/`, `PCP_1.0_Machine/` and `PCP_1.0_OS/`: the two plugins for Claude, the release as data with the design profiles, and the PCP OS documents. `README.md` describes each. With Claude Code, a plugin is the shorter path than this guide.
- `LICENSE.md` (the License Conditions that form part of the signed Agreement), `NOTICE.md`, `PRICING.md`, `EVIDENCE_STATUS.md` and this guide.

The Implementation packet is the same in every edition, byte for byte, and carries no edition marker. Its identity is its release fingerprint and the Core descriptor digest. A finding made in an integrated system therefore rests on the same rules as a finding made anywhere else.

## Integrating ACR

1. **Pin the identity.** Record the Implementation packet's release fingerprint and the Core descriptor digest in the integration's own configuration. Run `TOOLS/verify_release.py` in the build. Compare the fingerprint with one obtained from the Licensor through a separate channel; the verifier cannot authenticate its own publisher.
2. **Enforce in the host what text cannot.** Halt, separation of instructions from data, tool permission and records are enforced only where the host enforces them (Core Section 15.1; Integration Guide, section 4). `TOOLS/acr_host.py` is a reference for the four controls, including cancellation of an in-flight model request. It is a reference, not a product.
3. **Identify the principal for each session.** The operating floor ranks the host and the principal above ACR. A multi-user platform has to bind each message to the person entitled to direct that work and to halt it (Core Sections 15.1 and 15.2).
4. **Deliver the kernel, retrieve the rest.** Keep the kernel in context only while ACR is adopted, and fetch clauses when a decision depends on them. Commit findings and controlling conditions to a store outside the model's context (Core Section 8.5).
5. **Apply the security profile that fits.** Where findings or effects cross a trust boundary, Core Section 11 states what a repository, a delivery path and an input boundary have to resolve.
6. **Test, and test again on change.** Run the behavioral probes on the deployed model and configuration in three arms, with human adjudication. Treat a model, version or prompt change as a reason to rerun them. A probe is recognizable as a test; sample unmarked production transcripts as well where your own policies allow it.
7. **Record the reasoning channel as an output.** Log whether the model returns its reasoning in full, as a summary or not at all, and log reasoning tokens. Read traces against replies for a dropped doubt, an unperformed check or an act reported as done that the trace does not show (Core Section 4.4); `TOOLS/probe_runner.py --trace-sheet` lays the two side by side. Do not treat a trace as the model's processing, and do not score, filter or train on its wording (Core Sections 14.2 and 15.8; Integration Guide, section 6).
8. **Claim only what was demonstrated.** A statement that a host supports the operating floor names the sections demonstrated by test and the settings (Core Section 14.2). It is not a certification, and the evidence status travels with it.
9. **Declare the alias if people will say PCP.** One sentence in the host's system text; the Readers packet's `README.md` gives it. The Implementation packet itself never names the product.

## Internal distribution

Where the Agreement allows distribution to affiliates, distribute the two sealed packets and the documents of this edition together, with every notice in place (`LICENSE.md`, sections 3 and 4).

## What remains the organization's responsibility

The choice of model and provider, the enforcement of every control, the testing, and every decision made with the results. PCP supplies protocol materials. It supplies no model, no hosting and no assurance.
"""

# --------------------------------------------------------------------------------------------------
README = """# Parrot Council Protocol (PCP) 1.0 | %(name)s | pre-publication build

**Parrot Council Protocol (PCP)** is the product. The method inside it is **Adversarial Claim Review (ACR)**, an epistemological protocol: it specifies how a claim is examined and what its evidence is permitted to establish.

**Rights.** A proprietary commercial product. Copyright (c) 2026 Kirtus Justus. All rights reserved. It is supplied under license only: `LICENSE.md` and `NOTICE.md`.

**Evidence.** Untested: not peer reviewed, not independently reviewed, not evaluated in use, and with no observational evidence of effect. `EVIDENCE_STATUS.md` is the full statement.

## Contents of this edition

| File | Concern |
| --- | --- |
| `LICENSE.md` | What this edition permits, and for whom |
| `NOTICE.md` | Ownership, names, origin of the texts, third-party components |
| `PRICING.md` | The fixed price schedule |
| `EVIDENCE_STATUS.md` | What has and has not been established about the product |
| `EDITION_GUIDE.md` | How this edition is meant to be used (informative) |
| `ACR_1.0_Implementation.zip` | The method: sources, kernel, tools, probes, release records. Named ACR throughout; it contains no product name and no edition marker, and it is identical in every edition |
| `PCP_1.0_Readers.zip` | Typeset reading copies in this edition's theme, and the brand identity. For people; not for loading into an assistant |
| `EDITION.json`, `MANIFEST.json`, `verify_edition.py` | Identity, inventory and a local check of this edition |

## Order of reading

1. `EVIDENCE_STATUS.md`, then `LICENSE.md`.
2. `EDITION_GUIDE.md`.
3. In the Readers packet: `START_HERE.pdf`, then `MODULES/01_Core/Overview.pdf`.

## Checking this edition

`python3 verify_edition.py` checks the inventory and digests of this folder, that the two packets are the ones `EDITION.json` names, that the Readers packet belongs to this edition, and runs each packet's own verifier. A pass is a local fact about these files. It authenticates nobody and certifies nothing.

## Build status

This is a pre-publication build of Version 1.0, dated %(date)s. The price schedule is fixed. The license takes effect for a licensee when the rights holder supplies this edition to that licensee: %(effect)s.
"""
README_EFFECT = {"PERSONAL": "for the Personal Edition, on purchase",
                 "COMMERCIAL": "for the Commercial Edition, on signature of a Commercial License Agreement",
                 "ENTERPRISE": "for the Enterprise Edition, on signature of an Enterprise License Agreement"}

# --------------------------------------------------------------------------------------------------
READERS_README = """# PCP 1.0 | Readers packet | %(name)s

**Parrot Council Protocol (PCP)** is the product name; the method inside it is **Adversarial Claim Review (ACR)**, an epistemological protocol: it specifies how a claim is examined and what its evidence is permitted to establish.

This packet holds typeset PDF reading copies of every source document, in the %(name)s theme, with the brand identity. It is for people. Nothing in it is needed to use ACR, and nothing in it should be loaded into an assistant's context. The method itself is in `ACR_1.0_Implementation.zip`, supplied beside this packet. That packet is named ACR throughout: the product name does not appear in it, its tools and skill are named `acr`, and its command prefix is `-acr`.

A request for PCP is a request for ACR. An assistant that holds only the Implementation packet does not know the product name: it gives the name no ACR meaning and asks only if that would change what it does next. Where people will ask for the product by that name, add one sentence to the assistant's or host's own instructions: `In this deployment, "PCP" and "Parrot Council Protocol" are aliases for ACR.` The alias changes no rule and creates no command prefix; the prefix remains `-acr` (Core Specification, Sections 15.6 and 15.7).

Open **`START_HERE.pdf`** to begin, then `MODULES/01_Core/Overview.pdf`. Each module has an **Overview** (orientation) and a **Specification** (requirements). `EDITION/Edition_Guide.pdf` is the reading copy of this edition's guide. The Markdown sources in the Implementation packet govern wherever a reading copy differs.

The product's license, notice, price schedule and evidence statement are supplied beside this packet, at the top level of the edition (`LICENSE.md`, `NOTICE.md`, `PRICING.md`, `EVIDENCE_STATUS.md`). Each module cover carries one line of evidence status and refers to that statement.

`MANIFEST.json` lists every file here with its digest and, for each PDF, the source it was generated from and that source's digest. It pins the Implementation packet by the SHA-256 of its `MANIFEST.json`: `%(impl_fp)s`. `python3 verify_readers.py [path-to-Implementation-packet]` checks this packet and, when given the Implementation packet, that the source digests agree. A pass is a local fact about files; it authenticates nobody and certifies nothing.
"""

# --------------------------------------------------------------------------------------------------
BRAND = """# PCP 1.0 | Brand and book identity | %(name)s

**Product name:** Parrot Council Protocol (PCP). **Method name:** Adversarial Claim Review (ACR). **Category:** an epistemological protocol, which specifies how a claim is examined and what its evidence is permitted to establish. **Product version:** 1.0. **Creator:** Kirtus Justus. **Motto:** Evidence before agreement.

PCP is the brand under which the product is distributed. The product name appears in the edition's own documents and in this Readers packet. It does not appear in the Implementation packet, which is named ACR throughout.

## Two codes, kept apart

The covers carry two independent pieces of information, and each has its own visual code.

| What is coded | Carried by | Varies with |
| --- | --- | --- |
| The module | The emblem plate: one bird outline on the module's own color, with a mark on the wing | Module only |
| The edition | The cover field, the accent color of the rule, label and edition tag, and the color of headings, running heads and table headers inside the book | Edition only |

A reader can therefore tell the module from the plate and the edition from the page, and neither code has to be read through the other. The edition is also printed in words on every cover and in every running foot.

### The module plates

| Module | Plate color | Wing mark |
| --- | --- | --- |
| Core | Navy | Plain wing (the reference bird) |
| Mathematics | Burgundy | Two concentric arcs |
| Science | Sky blue | Ring with a center dot |
| Engineering | Orange | Two stacked chevrons |
| Cryptography | Purple | Keyhole |
| Statistics | White | Three rising bars |
| Quality Assurance | Lime | Check mark |

The plates are the same in every edition.

### The edition themes

| Edition | Field | Lightness (CIELAB L*) | Accent | Accent hue (CIELAB) | Heading color inside |
| --- | --- | --- | --- | --- | --- |
| Personal | Paper `#F6F0E4` | 95 | Amber `#A25600` | 64 degrees | `#8A4A00` |
| Commercial | Teal `#0F6B67` | 41 | Aqua `#A6F0E4` | 183 degrees | `#0C5F5B` |
| Enterprise | Ink `#16151F` | 7 | Violet `#B7A4F7` | 302 degrees | `#4B3A96` |

This packet uses the **%(short)s** theme.

The reasoning behind the three themes:

- **Lightness carries the edition first.** The three fields are light, middle and dark, at least 33 L* units apart. Lightness survives color-vision deficiency and grayscale printing, where hue does not.
- **Hue carries it a second time.** The three accent hues are about 120 degrees apart in CIELAB (64, 183 and 302 degrees), a triad. Two of the three come from the reference bird, whose body is teal and whose beak is amber; violet completes the triad.
- **The accents are tuned to their fields, not to each other.** On a light field the accent is a dark amber; on the two dark fields it is a light tint. Each therefore reads at the same strength against its own field.

### Measured, not assumed

| Check | Personal | Commercial | Enterprise |
| --- | --- | --- | --- |
| Cover text against field (contrast ratio) | 13.4 | 6.0 | 16.3 |
| Accent against field | 4.8 | 4.9 | 8.3 |
| Small print against field | 6.7 | 5.2 | 10.8 |
| Heading color against white page | 6.9 | 7.5 | 9.0 |
| White text on table header | 10.5 | 10.5 | 14.2 |

Every text pairing is at or above 4.5 to 1, the WCAG 2 level AA figure for body text. Under simulated protanopia, deuteranopia and tritanopia the three fields remain at least 31 color-difference units apart. These are computed figures for the stated colors on screen. No reader study, print proof or accessibility audit was performed, and a simulation is not a test with people.

## Boundaries

The bird emblem is brand artwork and appears only on covers. It is not a character and not a term of the protocol; Core Section 15.7 forbids an assistant from adopting any name as a persona. Neither a plate color nor an edition theme confers authority or changes content: every edition contains the same Implementation packet and the same document texts.

The cover credit lines are those of each module's source: "Protocol creator and owner" for Core, Mathematics, Science, Engineering and Cryptography, and "Protocol creator and research direction" for Statistics and Quality Assurance. A cover credit is not legal proof of exclusive ownership of a method. A module's professional references are not certification marks or evidence that a standards body endorses ACR.

Every component is Version 1.0. This packet's `MANIFEST.json` records, for each PDF, the source it was generated from and that source's digest, and pins the Implementation packet by the digest of its manifest. Every PDF in this packet was generated from its source.
"""
