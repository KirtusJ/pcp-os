//! Return mail is indexed by the existing postmark bits. The sorting office's
//! welcome card is always in the box, including on a courier's first night.
use crate::{Route, ROUTES};

pub const CARD_COUNT: usize = 10;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Picture,
    Message,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mailbox {
    pub card: usize,
    pub side: Side,
}
impl Mailbox {
    pub const fn new() -> Self {
        Self {
            card: 0,
            side: Side::Picture,
        }
    }
    pub fn received(card: usize, album: u32) -> bool {
        card == 0 || (card < CARD_COUNT && album & (1 << (card - 1)) != 0)
    }
    pub fn browse(&mut self, forward: bool, album: u32) {
        let mut next = self.card;
        for _ in 0..CARD_COUNT {
            next = (next + if forward { 1 } else { CARD_COUNT - 1 }) % CARD_COUNT;
            if Self::received(next, album) {
                break;
            }
        }
        self.card = next;
        self.side = Side::Picture;
    }
    pub fn flip(&mut self) {
        self.side = match self.side {
            Side::Picture => Side::Message,
            Side::Message => Side::Picture,
        };
    }
}
pub struct Card {
    pub place: &'static str,
    pub title: &'static str,
    pub sender: &'static str,
    pub lines: &'static [&'static str],
}
impl Card {
    pub fn for_index(i: usize) -> &'static Self {
        &CARDS[i]
    }
    pub fn address(i: usize) -> Option<(Route, usize)> {
        if i == 0 || i >= CARD_COUNT {
            None
        } else {
            Some((ROUTES[(i - 1) / 3], (i - 1) % 3))
        }
    }
}
const CARDS: [Card; CARD_COUNT] = [
    Card {
        place: "Sorting Office",
        title: "The spare key",
        sender: "Nell, night sorter",
        lines: &[
            "Little blue courier,",
            "",
            "Your hook is the third one down.",
            "I moved it lower after I saw you",
            "trying to hang up that scarf.",
            "",
            "There's a spare key in the blue",
            "teapot. Come in even if I'm out.",
        ],
    },
    Card {
        place: "Pine Key",
        title: "Two cups",
        sender: "Ada, light keeper",
        lines: &[
            "My sister sent her soup recipe.",
            "She measures pepper in sneezes.",
            "I made it exactly as written.",
            "",
            "Next time you pass the lamp,",
            "there's a cup on the sill.",
            "The little one is yours.",
        ],
    },
    Card {
        place: "Moth Bank",
        title: "The night census",
        sender: "Kit, age nine",
        lines: &[
            "You brought my counting book!",
            "I got up to seventeen moths.",
            "Then one landed on seventeen",
            "and I lost my place.",
            "",
            "I drew that one for you.",
            "Look at his ridiculous ears.",
        ],
    },
    Card {
        place: "Gull Rock",
        title: "One silver spoon",
        sender: "Bess at the galley",
        lines: &[
            "That spoon in the photograph",
            "is definitely ours. The dent",
            "is where I opened a paint tin.",
            "",
            "If the gull comes back, tell him",
            "he can keep it. I'd quite like",
            "the other eleven back, though.",
        ],
    },
    Card {
        place: "Minnow",
        title: "The smallest catch",
        sender: "Orrin, deckhand",
        lines: &[
            "My boy traced his hand on the",
            "back of the tide table you",
            "brought. I put mine over it.",
            "",
            "The difference is one knuckle",
            "less than it was last winter.",
            "I've asked for the early boat.",
        ],
    },
    Card {
        place: "Paper Moon",
        title: "Side B",
        sender: "June, radio operator",
        lines: &[
            "That was the missing needle.",
            "After three months of silence",
            "we finally played side B.",
            "",
            "You should have seen the cook",
            "waltz with a sack of onions.",
            "He took his hat off first.",
        ],
    },
    Card {
        place: "Last Ferry",
        title: "Seat by the stove",
        sender: "Sal, night purser",
        lines: &[
            "One passenger tonight. No fare,",
            "four feet, asleep on the mailbag.",
            "Your letter says her name is Pip.",
            "",
            "I called it once. One ear moved.",
            "I think that settles it.",
            "I'll take her home at dawn.",
        ],
    },
    Card {
        place: "Juniper",
        title: "Something green",
        sender: "Ivo, balloon rigger",
        lines: &[
            "The cutting made it up intact.",
            "I planted it in an old cocoa tin",
            "and lashed it to my bunk.",
            "",
            "Everything else in this cabin",
            "swings when we change course.",
            "Now something grows here too.",
        ],
    },
    Card {
        place: "Alcyone",
        title: "A familiar sky",
        sender: "Mara, navigator",
        lines: &[
            "My father has drawn the stars",
            "as he sees them from his chair.",
            "One bright point is out of place.",
            "",
            "Took me half the watch to see it.",
            "It's our kitchen window.",
            "I put it on my chart in red.",
        ],
    },
    Card {
        place: "North Star",
        title: "The red square",
        sender: "Em, first watch",
        lines: &[
            "Mum sent the last quilt square.",
            "It used to be my brother's coat.",
            "You can still see a buttonhole.",
            "",
            "I sewed it in over the pole.",
            "My knees are warm. Tell her",
            "exactly that. She'll want to know.",
        ],
    },
];
