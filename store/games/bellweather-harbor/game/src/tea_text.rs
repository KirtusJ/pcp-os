//! The emergency committee's order paper, receipts and loose minutes.

pub struct Granny {
    pub name: &'static str,
    pub role: &'static str,
    pub line: &'static str,
}

pub struct Motion {
    pub title: &'static str,
    pub sponsor: usize,
    pub proposal: &'static str,
    pub receipt: &'static str,
}

pub const GRANNIES: [Granny; 3] = [
    Granny {
        name: "Beatrice Brine",
        role: "Chair",
        line: "Is anyone here against tea? Good. Is anyone under the table against tea?",
    },
    Granny {
        name: "Vera Doig",
        role: "Ship's carpenter",
        line: "My folding chairs don't fold with people in them. I've had the letters.",
    },
    Granny {
        name: "Maud Spooner",
        role: "Minutes secretary",
        line: "Speak into my good ear, dear. The other one's for amendments.",
    },
];

pub const TREATY_TITLE: &str = "Standing Order Thirteen";
pub const PROGRAMME_NOTE: &str =
    "Three grandmothers. Four chairs. Something under the fourth is quietly wringing out the cushion.";

pub const MOTIONS: [Motion; 6] = [
    Motion {
        title: "Dry the fourth chair",
        sponsor: 1,
        proposal: "Bag the salt in my old stockings. I'll draw the damp out of that cushion.",
        receipt: "Vera sits down to test it. A teaspoon of seawater lands neatly in her breast pocket.",
    },
    Motion {
        title: "Give the damp an address",
        sponsor: 2,
        proposal: "I can't keep writing 'wet bit'. Lay a line of salt round it so I can put it on the plan.",
        receipt: "The puddle moves six inches to the left. Maud rubs out a wall and draws a little porch.",
    },
    Motion {
        title: "Measure for a visitor",
        sponsor: 1,
        proposal: "Dust the seat with salt. When our guest gets up, I'll finally know how wide to make it.",
        receipt: "The print is a perfect chart of the harbor. Vera lets the seat out two inches at the lighthouse.",
    },
    Motion {
        title: "See to the sandwiches",
        sponsor: 0,
        proposal: "One plate at each place, please. And cut the crusts off for our guest. It hasn't any teeth.",
        receipt: "Three plates come back with crumbs. The fourth comes back polished, with a mussel holding the crusts.",
    },
    Motion {
        title: "Set a place for the sea",
        sponsor: 2,
        proposal: "The guest has signed for its tea. Beatrice, turn the register sideways. It's still signing.",
        receipt: "Maud sets a card marked SEA at the fourth place. The chair pulls itself out, all four feet squeaking.",
    },
    Motion {
        title: "Ask how it takes its tea",
        sponsor: 0,
        proposal: "The sea may take its seat. Hands up for one lump. Oh, for goodness' sake, a little wave will do.",
        receipt: "The sea sits. Northstar bobs up a foot. One small wave lifts a sugar lump into its cup. Maud records the vote.",
    },
];

// These attend to the current watch while the completed receipts stay on file.
// They work at any stage: none assumes a motion has already passed.
pub const TIDE_MINUTES: [&str; 12] = [
    "Beatrice counts the cups twice. One has a handle on the inside. Vera trades with her.",
    "Maud puts a full stop after 'milk'. A periwinkle takes it halfway down the page.",
    "Vera's left boot squeaks on every third step. Her right answers from the coat cupboard.",
    "A spoon stands up in the jam. Beatrice gives it the floor. It slowly sits down.",
    "Kit brings a letter addressed to Whoever Keeps Moving the Pier. Vera pockets it.",
    "Maud writes 'gentle swell'. The table gives a little bow. She underlines 'gentle'.",
    "The sugar tongs turn up in Nell's lamp room. The wick has two very tidy pleats.",
    "A gull takes the butter knife. Vera says she'll worry when it comes back for a fork.",
    "Ada sends six buns and a note asking for her plate back. The plate has signed the note too.",
    "Beatrice warms the pot. Something knocks from inside. She says it can wait its turn.",
    "A shrimp falls out of Maud's pencil case. She gives it the short pencil to take home.",
    "Vera takes a biscuit for the walk. The biscuit takes a second biscuit for the walk.",
];

pub const KEEP_TEXT: &str =
    "Beatrice lays your place. Out by Northstar, a wave lifts its best saucer. It knows where you sit.";
pub const ADJOURN_TEXT: &str =
    "Vera folds three chairs. The sea takes its own home. From beyond the breakwater comes one last scrape of wood.";

pub const SOUVENIR_TITLE: &str = "The fourth teaspoon";
pub const SOUVENIR_TEXT: &str =
    "A little spoon with four thumbprints in the silver. Three have whorls. The fourth has tiny depth soundings.";
