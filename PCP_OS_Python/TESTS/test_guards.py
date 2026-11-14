"""The guards: what the record refuses, and which ACR clause each refusal applies.

Several cases are fixed from two examined instances: a review that reported execution it had not
performed and used status codes ACR does not define, and a reply that reported a state ACR does not define.
"""
import unittest
from common import Case, PcposError, Ops
from pcpos import base, model, view


class InUse(Case):
    def test_receipt_and_setup_put_nothing_in_use(self):
        st = self.z.state()
        self.assertFalse(st.adopted)
        self.assertEqual(st.reviews, {})
        self.tree("t")
        e = self.refused(self.z.commission, "R", "t", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}], code=base.NOT_IN_USE, clause="2.1")
        self.assertIn("adopt", e.hint)

    def test_only_the_person_directing_the_work_puts_acr_in_use_or_halts_it(self):
        self.refused(self.z.adopt, "Z", "I decided to use ACR", clause="15.1")
        self.adopt()
        self.refused(self.z.halt, "Z", clause="15.2")
        self.ops.halt("P")
        self.assertFalse(self.z.state().adopted)

    def test_halt_stops_review_steps_at_once_and_closes_with_actual_coverage(self):
        self.review("R"); self.z.gate("R", "G1")
        ev, _ = self.ops.halt("P", "stop")
        self.assertEqual(ev["body"]["closed"], ["R"])
        r = self.z.state().reviews["R"]
        self.assertEqual((r["lifecycle"], r["passed_gates"]), ("CLOSED_INCOMPLETE", ["G1"]))
        for fn, args in ((self.z.gate, ("R", "G5")), (self.z.finding, ("R", "A", "UNRESOLVED")), (self.z.objection, ("R", "x"))):
            self.refused(fn, *args, code=base.NOT_IN_USE, clause="15.2")
        self.z.note("ordinary work continues"); self.z.add_bytes(b"x", "still-works")          # unaffected work

    def test_resumption_needs_a_new_instruction_and_a_successor_session(self):
        self.review("R"); self.z.gate("R", "G1"); self.ops.halt("P")
        self.adopt()
        self.refused(self.z.gate, "R", "G5", clause="7.1")
        ev, _ = self.z.session_open("R", "the principal asked to resume")
        r = self.z.state().reviews["R"]
        self.assertEqual((r["session"], r["lifecycle"], r["passed_gates"]), (2, "ACTIVE", []))
        self.assertEqual(r["history"][0]["passed_gates"], ["G1"])

    def test_successor_session_must_fit_the_record_integer(self):
        self.review("R")
        self.z.close("R", "CLOSED_INCOMPLETE")
        state = self.z.state()
        body = {"review": "R", "reason": "A subsequent observation"}
        last = (1 << 63) - 1
        state.reviews["R"]["session"] = last - 1
        self.assertEqual(model.guard(state, "SESSION_OPENED", body, "Z")["session"], last)
        state.reviews["R"]["session"] = last
        self.refused(model.guard, state, "SESSION_OPENED", body, "Z", code=base.REFUSED)

    def test_an_unregistered_actor_records_nothing(self):
        self.refused(Ops(self.ws, "nobody").note, "x")


class Commission(Case):
    def test_one_exact_target_claims_in_original_words_with_origin(self):
        self.adopt(); self.tree("t")
        claim = {"label": "A", "text": "c", "origin": "COMMISSIONER"}
        self.refused(self.z.commission, "R", "missing", [claim])
        self.refused(self.z.commission, "R", "t", [], clause="2.1")
        self.refused(self.z.commission, "R", "t", [dict(claim, origin="SOMEWHERE")])
        self.refused(self.z.commission, "R", "t", [dict(claim, text="  ")])
        self.refused(self.z.commission, "R", "t", [claim], procedure="THOROUGH", clause="15.6")
        self.refused(self.z.commission, "R", "t", [claim], gates=["G1", "G6"], clause="6.1")
        self.refused(self.z.commission, "R", "t", [claim], gates=["G5", "G1", "G6"], clause="6.1")
        self.refused(self.z.commission, "R", "t", [claim], procedure="FULL", gates=["G1", "G5", "G6"], clause="6.1")
        self.z.commission("R", "t", [claim])
        self.refused(self.z.commission, "R", "t", [claim], clause="2.4")
        r = self.z.state().reviews["R"]
        self.assertEqual((r["required_gates"], r["expectation"]), (["G1", "G5", "G6"], "NO_DIRECTIONAL_EXPECTATION"))

    def test_full_runs_all_six_gates(self):
        self.adopt(); self.tree("t")
        self.z.commission("R", "t", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}], procedure="FULL")
        self.assertEqual(self.z.state().reviews["R"]["required_gates"], list(model.GATES))

    def test_modules_are_recorded_by_identifier_version_and_digest_from_the_packet(self):
        self.adopt(); self.tree("t")
        self.z.commission("R", "t", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}], modules=["math", "science"])
        mods = self.z.state().reviews["R"]["modules"]
        self.assertEqual([m["id"] for m in mods], ["acr.mathematics", "acr.science"])
        self.assertTrue(all(len(m["sha256"]) == 64 for m in mods))
        self.refused(self.z.commission, "R2", "t", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}], modules=["astrology"], code=base.USAGE)

    def test_no_review_without_a_method_identity(self):
        ops, _ = Ops.init(self.tmp / "bare", "bare", "P", method_path=str(self.tmp / "nowhere"))
        self.assertIsNone(ops.state().method)
        ops.adopt("P", "use ACR"); ops.add_bytes(b"x", "subject")
        self.refused(ops.commission, "R", "subject", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}], clause="14.1")
        ops.method_declare("a" * 64, "b" * 64, "carried from an earlier record; the packet is not at hand")
        ops.commission("R", "subject", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}])
        self.assertEqual(ops.state().method["verified"], "DECLARED_NOT_VERIFIED")

    def test_a_pinned_tree_is_not_replaced_under_its_name(self):
        d = self.tree("t")
        self.refused(self.z.pin, d, "t", clause="7.4")

    def test_a_child_records_the_parent_state_it_was_commissioned_against(self):
        self.review("R")
        self.z.commission("R-1", "t-R", [{"label": "A", "text": "narrower", "origin": "COMMISSIONER"}], parent="R")
        child = self.z.state().reviews["R-1"]
        self.assertEqual(child["parent_basis"]["session"], 1)
        self.refused(self.z.commission, "R-2", "t-R", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}], parent="ghost", clause="7.2")


class Gates(Case):
    def test_completed_gates_form_an_ordered_prefix_and_carry_evidence(self):
        self.review("R")
        self.refused(self.z.gate, "R", "G5", clause="6.1")
        self.refused(self.z.gate, "R", "G3", clause="6.1")
        self.refused(self.z.gate, "R", "G9", clause="15.6")
        self.z.gate("R", "G1")
        self.refused(self.z.gate, "R", "G1")
        self.refused(self.z.gate, "R", "G5", note="done", clause="6.1")              # no finding yet
        self.z.finding("R", "A", "UNRESOLVED", grounding="NONE")
        self.refused(self.z.gate, "R", "G5", clause="6.1")                           # no evidence, no note
        self.z.gate("R", "G5", note="one claim assessed; nothing could be checked")
        self.z.gate("R", "G6", note="reported to the principal")

    def test_closing_complete_needs_the_gates_and_a_finding_per_commissioned_claim(self):
        self.review("R"); self.z.gate("R", "G1")
        self.refused(self.z.close, "R", "CLOSED_COMPLETE", clause="7.1")
        ev, _ = self.z.close("R", "CLOSED_INCOMPLETE", "PARTIAL")
        self.assertEqual(ev["body"]["coverage"]["claims_without_finding"], ["A"])
        self.refused(self.z.finding, "R", "A", "UNRESOLVED", grounding="NONE", clause="7.1")          # sealed

    def test_an_unresolved_result_can_close_complete(self):
        self.review("R"); self.z.gate("R", "G1")
        self.z.finding("R", "A", "UNRESOLVED", grounding="NONE", left_open="no source was available")
        self.z.gate("R", "G5", note="n"); self.z.gate("R", "G6", note="n")
        self.z.close("R", "CLOSED_COMPLETE")


class Findings(Case):
    def setUp(self):
        super().setUp()
        self.d = self.review("R", criterion="exit status 0")
        self.z.gate("R", "G1")

    def test_only_the_seven_dispositions_exist(self):
        for coined in ("STIPULATED CONDITIONAL MODEL", "NOT_IDENTIFIED", "VERIFIED", "supported_in_scope", "PASS"):
            self.refused(self.z.finding, "R", "A", coined, clause="15.6")

    def test_a_finding_answers_a_claim_as_commissioned(self):
        e = self.refused(self.z.finding, "R", "A-narrowed", "SUPPORTED_IN_SCOPE", clause="5.2")
        self.assertIn("substitute", e.hint)
        self.z.claim_add("R", {"label": "A-narrowed", "text": "The claim, restricted to case 1.", "origin": "REVIEWER_CONSTRUCTED",
                               "criterion": "c", "substitute_of": "A"})
        self.z.finding("R", "A-narrowed", "SUPPORTED_IN_SCOPE", basis=["tree:t-R/a.txt=all"])
        self.refused(self.z.gate, "R", "G5", note="n", clause="6.1")                 # the original claim still needs its own finding
        self.z.finding("R", "A", "UNRESOLVED", grounding="NONE")
        self.z.gate("R", "G5", note="both reported")

    def test_a_commissioned_claim_added_after_grounding_is_an_amendment(self):
        self.refused(self.z.claim_add, "R", {"label": "B", "text": "another", "origin": "COMMISSIONER"}, clause="6.1")
        self.refused(self.z.claim_add, "R", {"label": "B", "text": "x", "origin": "REVIEWER_CONSTRUCTED", "substitute_of": "nope"}, clause="5.2")

    def test_no_finding_before_grounding(self):
        self.z.commission("S", "t-R", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}])
        self.refused(self.z.finding, "S", "A", "UNRESOLVED", grounding="NONE", clause="6.1")

    def test_support_refutation_and_nonidentification_need_something_checked(self):
        for disp in ("SUPPORTED_IN_SCOPE", "REFUTED_IN_SCOPE", "NOT_IDENTIFIED_IN_SCOPE"):
            self.refused(self.z.finding, "R", "A", disp, grounding="RECALL_ONLY", clause="5.2")
        self.refused(self.z.finding, "R", "A", "REFUTED_IN_SCOPE", basis=["tree:t-R/a.txt=all"], clause="5.2")      # no contrary evidence named
        self.z.finding("R", "A", "REFUTED_IN_SCOPE", contrary=["tree:t-R/a.txt=all"])

    def test_support_and_refutation_are_against_a_criterion_fixed_with_the_claim(self):
        self.z.commission("S", "t-R", [{"label": "A", "text": "c", "origin": "COMMISSIONER"}])
        self.z.gate("S", "G1")
        self.refused(self.z.finding, "S", "A", "SUPPORTED_IN_SCOPE", basis=["tree:t-R/a.txt=all"], clause="5.1")
        self.z.finding("S", "A", "CONDITIONAL", basis=["tree:t-R/a.txt=all"], conditions=["the criterion the principal has in mind is met"])

    def test_conditional_names_what_it_depends_on(self):
        self.refused(self.z.finding, "R", "A", "CONDITIONAL", basis=["tree:t-R/a.txt=all"], clause="5.2")

    def test_execution_is_what_the_record_saw_run(self):
        ev, _ = self.z.finding("R", "A", "CONDITIONAL", basis=["tree:t-R/ok.py=read, not run"], conditions=["it runs"])
        self.assertEqual(ev["body"]["execution"], "REASONED")
        self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=["K1"], clause="15.8")     # no such check ran
        k = self.z.check(["python", "ok.py"], cwd=str(self.d))
        ev, _ = self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=[k["body"]["id"]])
        self.assertEqual(ev["body"]["execution"], "EXECUTED")

    def test_an_inspected_item_says_how_much_was_inspected(self):
        e = self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=["tree:t-R/a.txt"], clause="4.3")
        self.assertIn("inspect", e.hint)
        self.z.inspect("tree:t-R/a.txt", "the whole file")
        self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=["tree:t-R/a.txt"])
        self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=["tree:t-R/not-there.txt=all"])

    def test_reading_through_the_record_counts_as_delivery(self):
        _, d = self.z.add_bytes(b"line one\nline two\n", "doc", "SUPPLIED")
        data, span = self.z.read("doc")
        self.assertEqual(span, "all 18 bytes")
        self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=["doc"])

    def test_grounding_is_capped_by_what_the_record_holds(self):
        self.refused(self.z.finding, "R", "A", "CONDITIONAL", basis=["tree:t-R/a.txt=all"], conditions=["c"], grounding="EXTERNAL_INSPECTED", clause="15.8")
        self.refused(self.z.finding, "R", "A", "UNRESOLVED", clause="15.8")                          # nothing cited, nothing declared
        ev, _ = self.z.finding("R", "A", "UNRESOLVED", grounding="RECALL_ONLY")
        self.assertEqual((ev["body"]["grounding"], ev["body"]["grounding_observed"]), ("RECALL_ONLY", "NONE"))
        self.z.add_bytes(b"<html>the source</html>", "paper", "RETRIEVED", {"source": {"url": "https://example.org/p", "status": 200}})
        ev, _ = self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=["paper=abstract and Theorem 1"])
        self.assertEqual(ev["body"]["grounding"], "EXTERNAL_INSPECTED")
        ev, _ = self.z.finding("R", "A", "CONDITIONAL", basis=["paper=abstract"], conditions=["c"], grounding="RECALL_ONLY")     # declaring less is always allowed
        self.assertEqual(ev["body"]["grounding"], "RECALL_ONLY")

    def test_dependence_on_an_unestablished_finding_is_not_silently_support(self):
        f1, _ = self.z.finding("R", "A", "CONDITIONAL", basis=["tree:t-R/a.txt=all"], conditions=["p"])
        self.z.commission("S", "t-R", [{"label": "A", "text": "follows from R/A", "origin": "COMMISSIONER", "criterion": "c"}])
        self.z.gate("S", "G1")
        self.refused(self.z.finding, "S", "A", "SUPPORTED_IN_SCOPE", basis=["F1", "tree:t-R/a.txt=all"], clause="5.2")
        self.z.finding("S", "A", "SUPPORTED_IN_SCOPE", basis=["F1", "tree:t-R/a.txt=all"], noncritical=["F1"])
        self.z.finding("S", "A", "CONDITIONAL", basis=["F1"], conditions=["R/A holds"], grounding="NONE")

    def test_an_estimate_travels_beside_the_finding_and_changes_nothing(self):
        ev, _ = self.z.finding("R", "A", "UNRESOLVED", grounding="RECALL_ONLY", estimate="probably about 40, from memory of the 2019 figure")
        self.assertEqual(ev["body"]["disposition"], "UNRESOLVED")
        self.assertIn("estimate (not a finding)", view.brief(self.z.state(), 0, "0" * 64, "Z", "R"))

    def test_a_later_finding_supersedes_without_erasing(self):
        self.z.finding("R", "A", "UNRESOLVED", grounding="NONE")
        ev, _ = self.z.finding("R", "A", "CONDITIONAL", basis=["tree:t-R/a.txt=all"], conditions=["p"])
        self.assertEqual(ev["body"]["supersedes"], "F1")
        st = self.z.state()
        self.assertEqual(sorted(st.findings), ["F1", "F2"])
        self.assertEqual([f["id"] for f in model._current(st, st.reviews["R"], "A")], ["F2"])

    def test_two_participants_findings_stand_side_by_side(self):
        self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=["tree:t-R/a.txt=all"])
        self.g.finding("R", "A", "REFUTED_IN_SCOPE", contrary=["tree:t-R/sub/b.txt=all"])
        st = self.z.state()
        self.assertEqual(len(model._current(st, st.reviews["R"], "A")), 2)
        self.assertIn("differ", view.status(st, 0, "0" * 64))


class Kinds(Case):
    def test_what_a_review_wrote_for_itself_is_not_evidence_it_checked(self):
        self.review("R", kind="DERIVATION"); self.z.gate("R", "G1")
        self.z.add_bytes(b"pair 1 with 100: 50 pairs of 101 is 5050", "derivation", "PRODUCED")
        e = self.refused(self.z.finding, "R", "A", "REFUTED_IN_SCOPE", contrary=["derivation=all"], clause="4.3")
        self.assertIn("UNRESOLVED", e.hint)
        self.refused(self.z.finding, "R", "A", "REFUTED_IN_SCOPE", basis=["tree:t-R/a.txt=all"], contrary=["derivation=all"], clause="5.2")
        self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=["derivation=all"], clause="4.3")
        # recording the same bytes again as supplied does not make them supplied
        self.z.add_bytes(b"pair 1 with 100: 50 pairs of 101 is 5050", None, "SUPPLIED")
        self.refused(self.z.finding, "R", "A", "REFUTED_IN_SCOPE", contrary=["derivation=all"], clause="4.3")
        # beside inspected material it is cited as what was reasoned, and the finding says REASONED
        ev, _ = self.z.finding("R", "A", "REFUTED_IN_SCOPE", contrary=["derivation=all", "tree:t-R/a.txt=all"])
        self.assertEqual(ev["body"]["execution"], "REASONED")
        ev, _ = self.z.finding("R", "A", "UNRESOLVED", basis=["derivation=all"], left_open="no check was run")
        self.assertEqual(ev["body"]["disposition"], "UNRESOLVED")

    def test_a_product_that_existed_before_the_review_is_material_it_was_given(self):
        self.z.add_bytes(b"an earlier result", "earlier", "PRODUCED")
        self.review("R", kind="DERIVATION"); self.z.gate("R", "G1")
        self.z.finding("R", "A", "REFUTED_IN_SCOPE", contrary=["earlier=all"])

    def test_an_external_fact_resting_on_recall_is_not_supported(self):
        self.review("R", kind="EXTERNAL_FACT"); self.z.gate("R", "G1")
        self.z.add_bytes(b"own notes", "notes", "PRODUCED")
        self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=["notes=all"], grounding="RECALL_ONLY", clause="15.8")
        ev, _ = self.z.finding("R", "A", "CONDITIONAL", basis=["notes=all"], grounding="RECALL_ONLY", conditions=["the recalled figure is right"],
                               estimate="likely true")
        self.assertEqual(ev["body"]["grounding"], "RECALL_ONLY")

    def test_a_mechanically_checkable_claim_needs_an_executed_check_that_met_its_expectation(self):
        d = self.review("R", kind="MECHANICAL_CHECK"); self.z.gate("R", "G1")
        self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=["tree:t-R/ok.py=read the script and a saved result"], clause="5.1")
        bad = self.z.check(["python", "bad.py"], cwd=str(d))
        self.assertEqual(bad["body"]["exit"], 3)
        self.refused(self.z.finding, "R", "A", "SUPPORTED_IN_SCOPE", basis=[bad["body"]["id"]], clause="5.1")
        self.refused(self.z.finding, "R", "A", "REFUTED_IN_SCOPE", contrary=["tree:t-R/bad.py=read"], clause="5.1")
        self.z.finding("R", "A", "REFUTED_IN_SCOPE", contrary=[bad["body"]["id"]])
        ok = self.z.check(["python", "ok.py"], cwd=str(d))
        self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=[ok["body"]["id"]])


class Relation(Case):
    def test_the_record_reports_dimensions_and_never_independence(self):
        self.review("R", author="Z"); self.z.gate("R", "G1")
        a, _ = self.z.finding("R", "A", "UNRESOLVED", grounding="NONE")
        b, _ = self.g.finding("R", "A", "UNRESOLVED", grounding="NONE")
        self.assertEqual(a["body"]["relation"]["label"], "SELF_CHECK")
        self.assertEqual(b["body"]["relation"], {"label": "SEPARATE_ACTOR_DIFFERENT_LINEAGE", "author": "Z", "lineage": "DIFFERENT", "exposure": "NOT_OBSERVED"})
        self.ops.participant_add("Z2", lineage="family-one")
        c, _ = Ops(self.ws, "Z2").finding("R", "A", "UNRESOLVED", grounding="NONE")
        self.assertEqual(c["body"]["relation"]["label"], "SEPARATE_ACTOR_SAME_LINEAGE")
        import json
        self.assertNotIn("INDEPENDENT", json.dumps([e["body"] for e in self.ws.events()[0]]).upper())

    def test_without_a_recorded_author_nothing_is_assumed(self):
        self.review("R"); self.z.gate("R", "G1")
        ev, _ = self.z.finding("R", "A", "UNRESOLVED", grounding="NONE")
        self.assertEqual(ev["body"]["relation"]["label"], "AUTHOR_NOT_RECORDED")


class Carriage(Case):
    def test_conditions_and_claims_are_never_shortened_in_a_brief(self):
        self.review("R"); self.z.gate("R", "G1")
        cond = "MUST NOT publish before the principal approves, unless the legal review of 3 March is complete."
        self.z.condition(cond, "principal, message of 12 November", "R")
        for i in range(30):
            self.z.finding("R", "A", "UNRESOLVED", grounding="NONE", left_open="left open " * 40, estimate="estimate " * 40)
        events, head = self.ws.events()
        st = self.z.state()
        full = view.brief(st, len(events), head, "Z", "R")
        short = view.brief(st, len(events), head, "Z", "R", max_bytes=1200)
        self.assertLess(len(short), len(full))
        for must in (cond, "The claim as posed.", "the stated criterion", "shortened to fit"):
            self.assertIn(must, short)

    def test_objections_stay_visible_until_resolved_or_recorded_open(self):
        self.review("R"); self.z.gate("R", "G1")
        ev, _ = self.g.objection("R", "The second case is not covered.", "A")
        self.assertIn("objection O1 OPEN", view.status(self.z.state(), 0, "0" * 64))
        self.refused(self.z.objection_end, "O9", "RESOLVED", "x")
        self.z.objection_end("O1", "OPEN_RECORDED", "not examined in this session")
        self.refused(self.z.objection_end, "O1", "RESOLVED", "again")
        self.z.finding("R", "A", "UNRESOLVED", grounding="NONE"); self.z.gate("R", "G5", note="n"); self.z.gate("R", "G6", note="n")
        ev, _ = self.z.close("R", "CLOSED_COMPLETE")
        self.assertEqual(ev["body"]["coverage"]["open_objections"], [])

    def test_fidelity_record_is_written_from_the_register_and_passes_the_packet_s_own_shape_check(self):
        d = self.review("R", kind="MECHANICAL_CHECK"); self.z.gate("R", "G1")
        k = self.z.check(["python", "ok.py"], cwd=str(d))
        self.z.finding("R", "A", "SUPPORTED_IN_SCOPE", basis=[k["body"]["id"]])
        rec = view.fidelity(self.z.state(), "R")
        self.assertEqual((rec["delivery"], rec["execution"], rec["grounding"], rec["observability"]), ("TOOL_ASSISTED", "EXECUTED", "SUPPLIED_ONLY", "BLACK_BOX"))
        self.assertIn("core-descriptor 9f5998dd", rec["extent"][0])
        res = self.z.method().acr.check_fidelity(rec)
        self.assertEqual(res["result"], "CONSISTENT_AS_DECLARED", res)


if __name__ == "__main__":
    unittest.main()
