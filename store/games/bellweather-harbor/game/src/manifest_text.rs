//! Documents from the Bellweather quay. Both editions draw from this file.

pub struct Lead {
    pub title: &'static str,
    pub source: &'static str,
    pub body: &'static str,
}

pub struct Case {
    pub title: &'static str,
    pub scene: &'static str,
    pub question: &'static str,
    pub leads: [Lead; 4],
    pub hint: &'static str,
    pub reveal: &'static str,
}

pub struct Conclusion {
    pub title: &'static str,
    pub benefit: &'static str,
    pub epilogue: &'static str,
}

pub const PAIRS: [[u8; 2]; 3] = [[0, 2], [1, 3], [0, 3]];

pub const CASES: [Case; 3] = [
    Case {
        title: "One crate, two slips",
        scene: "The salt is all there. So why is there a second receipt stuck to the bottom of the scale drawer? The clerk holds it to the lamp. The writing runs backwards.",
        question: "Which two slips put the same weighing on paper twice?",
        leads: [
            Lead {
                title: "Scale roll",
                source: "Quayside scale / original",
                body: "No. 614. One crate of salt, 41 lb. The scale needle caught and the clerk wrote 47 first, then looped the 7 into a 1. Only one entry sits between the two bell marks.",
            },
            Lead {
                title: "Lamp invoice",
                source: "Watch hut / accounts",
                body: "Two pints of oil for the second lamp. In the margin the keeper has written, in a hand entirely too large for the space, 'One for each end of the pier. Again.'",
            },
            Lead {
                title: "Backwards receipt",
                source: "Scale drawer / loose slip",
                body: "In the shaving mirror it reads 614, SALT, 41 lb. Even the loop of the altered 7 is there. The wet ink came off on paper headed KITCHEN STORES. Someone tore away the bottom half.",
            },
            Lead {
                title: "Relief slip",
                source: "West watch / pocket book",
                body: "The watch changed hands at six. Both keepers signed for one key and one shaving mirror. Neither signed for freight. Someone has drawn a splendid moustache on the printed key.",
            },
        ],
        hint: "A second lamp or a second signature could belong to any shift. Look for a serial number and a mistake that appears twice.",
        reveal: "One weighing, copied wet. The clerk squares the two scraps against the lamp. The cut edge of KITCHEN STORES fits a pad the night cook keeps downstairs.",
    },
    Case {
        title: "Supper on the night shift",
        scene: "Downstairs, Ada Pike takes the red lid off a pot. 'Communists? Yes, love. Bellweather branch. Spies? Somebody might have told me before I put the good spoons out.' Her queue reaches the stairs.",
        question: "Which two records show what the kitchen tickets actually bought?",
        leads: [
            Lead {
                title: "Red enamel lid",
                source: "Cellar stove / lost property",
                body: "A hammered sickle scratches the underside. There is also a pawnshop price and a long complaint about whoever put a hot lid face down in the butter.",
            },
            Lead {
                title: "Supper tickets",
                source: "Serving hatch / counterfoils",
                body: "Each counterfoil says ONE BOWL. Numbers 31 through 42 have yesterday's twin punch. Number 43 still has its ticket attached. Nobody has taken today's stack upstairs.",
            },
            Lead {
                title: "Branch minutes",
                source: "Union hall / Ada's copy",
                body: "The branch voted seven to two to buy a bigger pot together. The two dissenters wanted two smaller pots. The secretary recorded that a bigger ladle passed unanimously.",
            },
            Lead {
                title: "Galley tally",
                source: "Dish rack / closing book",
                body: "Twelve bowls out. Twelve bowls washed. The cook pasted the redeemed halves, 31 to 42, beside the count. Two holes in each. A note under 38 says 'no onion next time'.",
            },
        ],
        hint: "You want the stub kept at the hatch and the other half turned in for supper. Count both piles.",
        reveal: "Twelve tickets bought twelve suppers. Ada turns ticket 43 over. Someone used the blank back to blot a cable from the room upstairs. 'Mind the third stair. We still haven't voted on it.'",
    },
    Case {
        title: "A bird on the wire",
        scene: "Above the stove stands a handpress. Behind it, a wire taps the sill where two leads shared a socket. Ada sets the blotter beside the cable spool. Somebody has been buying your figures before the board opens.",
        question: "Which two papers identify who was paid to send the cargo figures?",
        leads: [
            Lead {
                title: "Cable spool",
                source: "Upstairs wire / sent copy",
                body: "FINCH SIX. SALT 614, 41 LB. NEXT DEPOT BID FOLLOWS. A second hand signs the transmission. The toll clerk stamped circuit 8 and charged sixpence at 5:52.",
            },
            Lead {
                title: "Printer's proof",
                source: "Handpress / first pull",
                body: "The headline reads WHO SETS THE PRICE OF SALT? The column beneath is blank. In blue pencil the printer asks, 'Can we at least find out before Thursday?'",
            },
            Lead {
                title: "Fog signal card",
                source: "Sill / coast pilot's chart",
                body: "Six strokes mean a pilot is wanted at the bar. The code is for a bell and the card is ten years old. Somebody has pressed it faithfully into service under the short leg of the desk.",
            },
            Lead {
                title: "Expense claim",
                source: "Carbon book / office copy",
                body: "Rook Freight pays two scouts. Vale copies the scales. Moss wires the bids. Claim FINCH SIX, circuit 8, toll sixpence. Both signed. Moss also claimed for a bun. The bun was refused.",
            },
        ],
        hint: "Find the sent message and the paid claim for that exact transmission. Check the call sign, circuit and toll.",
        reveal: "Vale and Moss have been selling tomorrow's cargo figures to Rook Freight. Ada pockets the carbons. There's one rent-free room left on the quay. It will fit a public press or a proper night kitchen. She puts the key in your hand.",
    },
];

pub const ENDINGS: [Conclusion; 2] = [
    Conclusion {
        title: "Print the ledger",
        benefit: "+1 coin every time you ring the harbor bell.",
        epilogue: "At six, the first prices go up where everyone can read them. A boatman buys the first sheet and uses it to wrap his breakfast. He comes back for another so he can read it. Every ring now sells an extra copy.",
    },
    Conclusion {
        title: "Run the night kitchen",
        benefit: "+1 coin every time the rowboat finishes a trip, including captain-run trips.",
        epilogue: "Ada has the stove up by midnight. The rowboat takes a hot supper across on each trip. The crew at the far bank pays a coin for delivery. On the wall hangs the old red lid. They bought the two smaller pots after all.",
    },
];
