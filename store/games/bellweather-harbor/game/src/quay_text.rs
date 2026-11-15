//! Words spoken on the public quay, and the things its three regulars give away.
use crate::{economy::Economy, manifest::Ending, neighbors};

pub const ROLES: [&str; 3] = [
    "Keeper of the lower light",
    "Night-shift cook",
    "Mail tender",
];
pub const PLACES: [&str; 3] = ["The lamp stairs", "The serving hatch", "The packet steps"];

pub const ANSWERS: [&str; 3] = [
    "Two bells from the lower light. Nell heard you.",
    "Three knocks on an enamel pot. Ada sends her regards.",
    "A short whistle and a long one. Kit answers from the packet steps.",
];

pub struct Keepsake {
    pub title: &'static str,
    pub body: &'static str,
}

pub const KEEPSAKES: [[Keepsake; 3]; 3] = [
    [
        Keepsake {
            title: "A little brass moon",
            body: "Nell's old wick gauge fits on a keyring. She cut a nick in it so she could set the lamp by thumb in the dark. On the back, in much smaller letters, is the name of the woman who taught her.",
        },
        Keepsake {
            title: "Two lights in pencil",
            body: "A chart so often unfolded it has gone soft as cloth. The lower light and the kitchen window line up from the channel. 'Lose your compass,' Nell has written, 'and you can still steer toward supper.'",
        },
        Keepsake {
            title: "The last match",
            body: "A single blue match in a glass tube. The note says, 'My first night on watch I struck twelve. These days one does it. Keep the spare for the next person who needs twelve.'",
        },
    ],
    [
        Keepsake {
            title: "Half a meal ticket",
            body: "On the back Ada has pencilled the recipe for her mother's dumplings. No weights. 'If the spoon stands up, somebody needs to go fetch a little more milk.'",
        },
        Keepsake {
            title: "The honorary spoon",
            body: "A battered teaspoon hangs on a length of red wool. One night the crew ate in shifts of six with five spoons. The last sailor drank straight from the bowl and endowed the kitchen with this one.",
        },
        Keepsake {
            title: "A place laid for you",
            body: "A square of oilcloth with a corner darned in red. Ada has stitched your berth number into it. Down in the cellar there is a patch of clean wood on the long table exactly this size.",
        },
    ],
    [
        Keepsake {
            title: "A feather in the fold",
            body: "The envelope holds one brilliant blue feather. Kit picked it out of the hemp by the mail net. Whoever sent it has pressed a tiny muddy three-toed print where the stamp ought to go.",
        },
        Keepsake {
            title: "The unposted card",
            body: "A picture of the Last Ferry with all the passengers blurred except one small dog. Kit could never find its address. Someone has written 'good dog' in four different hands.",
        },
        Keepsake {
            title: "Forwarding address",
            body: "A blank luggage tag in Kit's tiniest writing: 'If found at sea, send on to the harbor. Someone there will know whose it is.' For the first time since coming ashore, you believe that applies to you.",
        },
    ],
];

pub const FAVORS: [[&str; 3]; 3] = [
    [
        "The lamp glass has gone brown. Salt and a twist of damp cloth will bring it up. Have you a crate to spare?",
        "The relief keeper's come down with a cold. I'll take salt for the glass and a crab for his supper up in one trip.",
        "We're laying in stores for the lower light. Salt for the glass, provisions for the keeper. Both go up on the same rope.",
    ],
    [
        "I promised the crew pickled onions by supper. Two crates of salt and I can keep that promise.",
        "Look at that queue. Three crates of crab ought to carry us to the dinner whistle. I'll pick the shells myself.",
        "We're building a bench under the awning. Two timber crates and a set of fittings. I'm tired of feeding people standing up.",
    ],
    [
        "I've got a split fish barrel for a float and a mail net that won't stay put. Can I buy a crate of salt to weigh the foot down?",
        "The new mail net takes two salt weights and a timber spreader. I've got the knots practised on my bootlaces.",
        "The packet's off on a long run. A meal for the crew, a set of fittings for the loose hatch and I can let go her stern line.",
    ],
];

pub const THANKS: [[&str; 3]; 3] = [
    [
        "There. You can see the inside of the glass again. Hold out your hand. I've had something brass knocking about in this pocket for years.",
        "He'll tell you he was well enough to come down. He'll also eat every bit of that crab.",
        "See the lamp wink twice? That's the keeper letting us know it all came up dry.",
    ],
    [
        "Give those onions till supper. Here, take the good half of the ticket. There's a recipe on it.",
        "You brought enough for the late boat too. Sit down a minute. Can you smell the butter?",
        "Go on, sit. If that bench can take the two of us it can take a ferry crew.",
    ],
    [
        "Watch the next letter. Right in the middle! I've got something blue for your pocket.",
        "First time the net's opened square all the way across. Even the dog on the ferry looks impressed.",
        "Listen. That's our packet clearing the bar. I always stay till the second whistle.",
    ],
];

/// The clerk can point to each neighbor even between their calls at the public quay.
pub fn duty(tide: u64, person: usize) -> &'static str {
    const ROUNDS: [[&str; 4]; 3] = [
        [
            "Cleaning the lower lantern",
            "Checking the channel marks",
            "Trimming wicks on the lamp stairs",
            "Walking the lamp gallery",
        ],
        [
            "Putting the onions on",
            "Serving the repair crew",
            "Laying out bowls by the stove",
            "Keeping supper warm for the late boat",
        ],
        [
            "Sorting the first post",
            "Mending the net by the packet steps",
            "Meeting the last ferry",
            "Counting the outgoing mailbags",
        ],
    ];
    ROUNDS.get(person).map_or("", |r| r[neighbors::watch(tide)])
}

/// A greeting follows the world clock, with occasional remarks about things the player actually did.
pub fn greeting(game: &Economy, person: usize) -> &'static str {
    if person >= 3 {
        return "";
    }
    let tide = game.freight.tide;
    let watch = neighbors::watch(tide);
    let day = tide / 12;
    let met = game.neighbors.helped[person];
    if game.lanterns_sailing() {
        return [
            "If you see one go dark, give it a moment. They're passing behind the mooring posts.",
            "We put tea lights in the jam tins. I should like it recorded that somebody finally brought my tins back.",
            "I tried to address a letter to that little boat of lights. Nell says some things are better delivered by hand.",
        ][person];
    }
    if game.neighbors.served && neighbors::visitor(tide) == person {
        let tier = met.saturating_sub(1).min(2) as usize;
        return THANKS[person][tier];
    }
    let ending = game.manifest.ending;
    if (day + watch as u64) % 3 == 1 {
        if person == 1 && ending == Ending::Kitchen {
            return "That second little pot was the best thing we ever bought. Listen to them both bubble.";
        }
        if person == 2 && ending == Ending::Print {
            return "I've folded the paper so the rowers can read the prices with one hand. Took me three goes.";
        }
        if person == 0 && game.yard.completed & 4 != 0 {
            return "That's Northstar's lamp out beyond my own. Two lights to bring the crews home.";
        }
        if person == 1 && game.factories[0].running {
            return "Hear that knock from the cannery? The lid press is just a hair out of time. I could dance to it.";
        }
        if person == 2 && game.postmarks != 0 {
            return "I've seen the ink on your postmarks. Same blue that gets all over my cuffs.";
        }
    }
    if neighbors::weather(tide) == 1 && watch % 2 == 0 {
        return [
            "In fog I count from the bell to the echo off the ferry shed. Today you could about touch the echo.",
            "Mist's halfway down the chimney. Soup smells of the whole harbor this morning.",
            "Can't see the far bank. Good thing a mail net doesn't need to see the postman.",
        ][person];
    }
    if neighbors::weather(tide) == 2 && watch % 2 == 1 {
        return [
            "Rain makes the rails slick. One hand for the light, one hand for the ladder.",
            "Mind the knot in that awning. If it comes loose we'll be having a very thin soup.",
            "Oilcloth around the letters. Let it rain on the envelopes all it likes once they're indoors.",
        ][person];
    }
    if met > 0 && (day + watch as u64) % 2 == 1 {
        return [
            "I put the kettle on when I saw your bow come round. There's enough for two.",
            "Usual bowl? I've kept your place at the end where the table doesn't wobble.",
            "I thought that was you on the quay! I'd know the sound of your rowlocks anywhere.",
        ][person];
    }
    const HOURS: [[&str; 4]; 3] = [
        ["Morning. Listen a second. You can hear the oars before you can see who's pulling them.",
         "I've spent half an hour chasing that gull off the red buoy. He's back on the green one now.",
         "Wicks cut, matches dry. Stay a minute if you'd like to see the first lamp catch.",
         "Count the lit windows on the far bank. When there's only one, Ada's still up."],
        ["Mind the step. Peeling onions is perfectly respectable work to be doing before sunrise.",
         "That whistle's for the repair crew. By the time you hear the echo they'll have emptied this pot.",
         "Take the end of the table by the door. You can watch the boats without getting up.",
         "Last bowl stays on the stove until the late boat's rope is on the bollard."],
        ["Got a letter for the Last Ferry. Address just says 'where the dog sits'. It'll find him.",
         "A kingfisher's been sitting on my mail net as if it's expecting something.",
         "Hear her whistle? Two short. Better get down to the steps with the bag.",
         "I can tell the late packet by her wake. That and the dog, who barks two seconds before I see her."],
    ];
    HOURS[person][watch]
}

/// Fragments of a conversation heard from the quay. A complete circuit takes two harbor days.
pub fn exchange(tide: u64) -> (&'static str, &'static str) {
    const WORDS: [(&str, &str); 12] = [
        (
            "Nell: Who taught that gull the dinner whistle?",
            "Ada: Whoever it was owes me six breakfasts.",
        ),
        (
            "Ada: Has that onion come back with the empty bowls?",
            "Kit: It's on the ferry. Someone paid its fare.",
        ),
        (
            "Kit: Got a letter for the keeper. Where's she gone?",
            "Nell: Down here. I like to see who delivers them.",
        ),
        (
            "Nell: That's my oil can swinging from your handlebars.",
            "Kit: I know. It's the only bell that dog respects.",
        ),
        (
            "Ada: Two pots, three lids. Explain that to me.",
            "Nell: The small red one's the moon when Kit puts on a show.",
        ),
        (
            "Kit: The mailbag's a little heavy on one side.",
            "Ada: That's your lunch. Don't deliver it this time.",
        ),
        (
            "Nell: Is that my good blue matchbox on the water?",
            "Kit: Look how straight she sails.",
        ),
        (
            "Ada: Who left a bowl on the bollard?",
            "Nell: Fellow on the last boat's got both hands on the rope.",
        ),
        (
            "Kit: Somebody's marked this letter 'by moonlight'.",
            "Nell: Put it in your pocket. It'll get there.",
        ),
        (
            "Nell: Whose turn is it to wake the light keeper?",
            "Ada: She's standing here. You're talking to her.",
        ),
        (
            "Ada: Quiet out there tonight.",
            "Kit: Wait for it. That's the first oar coming back.",
        ),
        (
            "Kit: If I miss the last boat, can I sleep by the stove?",
            "Ada: Your blanket's been there all winter.",
        ),
    ];
    let index = (tide % 12 + ((tide / 12) % 2) * 5) as usize % WORDS.len();
    WORDS[index]
}
