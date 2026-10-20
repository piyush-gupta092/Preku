package app.preku.pip

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin

// Pip's expressions as data: a port of `desktop/src/pip/expressions.ts`. Keep the two in step:
// same moods, same numbers, so Pip feels like the same character on every device.

enum class PipMood {
    Idle, Happy, Focus, Alert, Sleepy, Celebrate, Sad,
    Dizzy, Dance,
    Excited, Laughing, Crying, Angry, Confused, Surprised, Shocked, Thinking, Curious, Shy, Proud,
    Bored, Worried, Playful, Loving, Embarrassed,
    RestEyes, Stretch, Sip, Breathe, Yawn,
}

enum class EyeKind { Open, Bright, Watery, Flat, Ring, Smile, Closed, Laugh, Wink, Heart, Spiral }

enum class MouthKind {
    None, Small, Smile, Smirk, Shy, Grin, Laugh, Sing, O, BigO, Frown, Tremble, Wavy, Flat, Tongue, Yawn, Pout, Awkward,
}

enum class BrowKind { None, Angry, Worried, Raised, OneUp }

/** Tears and sweat sit on the face; the rest float around Pip (only at full size). */
enum class Extra { Tears, Sweat, Hearts, Confetti, Question, Zzz, Anger, Sparkle, Exclaim, Thought, Notes }

enum class Hands { None, Cheeks, Chin, Face, Hips, Eyes, Cup }

typealias Motion = (time: Float) -> Float
typealias ArmMotion = (time: Float, side: Int) -> Float

data class Look(val x: Float, val y: Float)

data class Expression(
    val eyes: EyeKind,
    val mouth: MouthKind,
    val arms: ArmMotion,
    val eyeScale: Float = 1f,
    val eyeOpen: Float = 1f,
    val look: Look? = null,
    val brows: BrowKind = BrowKind.None,
    val blush: Float = 0.5f,
    val puff: Boolean = false,
    /** Leaf tips: positive droops, negative perks up. */
    val leaves: Float = 0f,
    val leafWiggle: Boolean = false,
    val hands: Hands = Hands.None,
    val bob: Motion? = null,
    val tilt: Motion? = null,
    val shake: Motion? = null,
    val stretch: Float = 1f,
    val breath: Motion? = null,
    val extras: Set<Extra> = emptySet(),
)

private fun still(angle: Float): ArmMotion = { _, _ -> angle }
private val HIDDEN = still(0.05f)
private const val PI_F = PI.toFloat()

/** One slow breath: 4 s in, 4 s out. */
const val BREATH_MS = 8000f

fun inhale(time: Float): Float = 0.5f - 0.5f * cos(2f * PI_F * time / BREATH_MS)

/** 0 = cup lowered, 1 = cup at the mouth; one sip every 3.2 s. */
fun sip(time: Float): Float = 0.5f - 0.5f * cos(2f * PI_F * time / 3200f)

val EXPRESSIONS: Map<PipMood, Expression> = mapOf(
    PipMood.Idle to Expression(EyeKind.Open, MouthKind.Small, { t, s -> 0.55f + sin(t / 700f + if (s > 0) 0.6f else 0f) * 0.12f }),
    PipMood.Happy to Expression(
        EyeKind.Bright, MouthKind.Smile, { t, _ -> 2.1f + sin(t / 260f) * 0.15f },
        eyeScale = 1.08f, bob = { t -> 0.3f * abs(sin(t / 220f)) },
    ),
    PipMood.Focus to Expression(EyeKind.Open, MouthKind.None, still(0.75f), eyeOpen = 0.55f),
    PipMood.Alert to Expression(EyeKind.Open, MouthKind.O, still(2.4f), eyeScale = 1.2f, leaves = -6f),
    PipMood.Sleepy to Expression(EyeKind.Closed, MouthKind.Yawn, still(0.2f), leaves = 10f, extras = setOf(Extra.Zzz)),
    PipMood.Celebrate to Expression(
        EyeKind.Smile, MouthKind.Grin, { t, s -> 2.6f + sin(t / 110f + if (s > 0) PI_F else 0f) * 0.4f },
        leaves = -6f, leafWiggle = true, bob = { t -> abs(sin(t / 160f)) }, extras = setOf(Extra.Confetti),
    ),
    PipMood.Sad to Expression(
        EyeKind.Open, MouthKind.Frown, still(0.1f),
        look = Look(0f, 1f), brows = BrowKind.Worried, leaves = 10f,
    ),
    PipMood.Dizzy to Expression(EyeKind.Spiral, MouthKind.Wavy, { t, s -> 1.3f + sin(t / 80f + s) * 1f }),
    PipMood.Dance to Expression(EyeKind.Smile, MouthKind.Sing, still(2.2f), leaves = -6f, extras = setOf(Extra.Notes)),
    PipMood.Excited to Expression(
        EyeKind.Bright, MouthKind.Grin, { t, s -> 2.7f + sin(t / 90f + if (s > 0) 1.5f else 0f) * 0.3f },
        eyeScale = 1.25f, leaves = -8f, leafWiggle = true, bob = { t -> abs(sin(t / 150f)) },
        extras = setOf(Extra.Sparkle),
    ),
    PipMood.Laughing to Expression(
        EyeKind.Laugh, MouthKind.Laugh, { t, _ -> 0.9f + sin(t / 60f) * 0.15f },
        blush = 0.7f, shake = { t -> sin(t / 45f) * 1.5f },
    ),
    PipMood.Crying to Expression(
        EyeKind.Watery, MouthKind.Tremble, { t, _ -> 0.25f + sin(t / 70f) * 0.05f },
        look = Look(0f, 0.6f), brows = BrowKind.Worried, leaves = 8f,
        shake = { t -> sin(t / 40f) * 0.5f }, extras = setOf(Extra.Tears),
    ),
    PipMood.Angry to Expression(
        EyeKind.Open, MouthKind.Pout, still(1f),
        eyeOpen = 0.5f, brows = BrowKind.Angry, blush = 0.9f, puff = true, leaves = -4f,
        shake = { t -> sin(t / 35f) * 0.6f }, extras = setOf(Extra.Anger),
    ),
    PipMood.Confused to Expression(
        EyeKind.Open, MouthKind.Wavy, { t, s -> if (s > 0) 2.9f + sin(t / 120f) * 0.15f else 0.4f },
        look = Look(0.4f, -0.3f), brows = BrowKind.OneUp, tilt = { 0.15f }, extras = setOf(Extra.Question),
    ),
    PipMood.Surprised to Expression(
        EyeKind.Open, MouthKind.O, still(2.3f),
        eyeScale = 1.3f, brows = BrowKind.Raised, leaves = -6f, extras = setOf(Extra.Exclaim),
    ),
    PipMood.Shocked to Expression(
        EyeKind.Ring, MouthKind.BigO, HIDDEN,
        eyeScale = 1.2f, brows = BrowKind.Raised, leaves = -10f, hands = Hands.Cheeks, stretch = 1.04f,
        extras = setOf(Extra.Exclaim),
    ),
    PipMood.Thinking to Expression(
        EyeKind.Open, MouthKind.Flat, { _, s -> if (s > 0) 0.05f else 0.5f },
        look = Look(0.7f, -1f), brows = BrowKind.OneUp, hands = Hands.Chin, extras = setOf(Extra.Thought),
    ),
    PipMood.Curious to Expression(
        EyeKind.Bright, MouthKind.Small, still(1.1f),
        eyeScale = 1.15f, leaves = -6f, leafWiggle = true, tilt = { t -> 0.1f + sin(t / 500f) * 0.03f },
    ),
    PipMood.Shy to Expression(
        EyeKind.Open, MouthKind.Shy, still(0.25f),
        look = Look(-1f, 0.5f), blush = 1f, leaves = 4f, tilt = { -0.06f },
    ),
    PipMood.Proud to Expression(
        EyeKind.Open, MouthKind.Smirk, still(1f),
        eyeOpen = 0.7f, look = Look(0f, -0.6f), leaves = -6f, hands = Hands.Hips, stretch = 1.05f,
        extras = setOf(Extra.Sparkle),
    ),
    PipMood.Bored to Expression(
        EyeKind.Flat, MouthKind.Flat, still(0.05f),
        look = Look(0.5f, 0.3f), leaves = 12f, stretch = 0.93f,
    ),
    PipMood.Worried to Expression(
        EyeKind.Open, MouthKind.Wavy, { t, s -> 0.6f + sin(t / 100f + if (s > 0) PI_F else 0f) * 0.25f },
        look = Look(0f, 0f), brows = BrowKind.Worried, shake = { t -> sin(t / 90f) * 0.8f },
        extras = setOf(Extra.Sweat),
    ),
    PipMood.Playful to Expression(
        EyeKind.Wink, MouthKind.Tongue, { t, s -> if (s < 0) 2.2f + sin(t / 120f) * 0.3f else 0.6f },
        leafWiggle = true, bob = { t -> 0.2f * abs(sin(t / 200f)) }, tilt = { 0.08f },
    ),
    PipMood.Loving to Expression(
        EyeKind.Heart, MouthKind.Smile, { t, _ -> 1.6f + sin(t / 400f) * 0.1f },
        blush = 0.9f, tilt = { t -> sin(t / 600f) * 0.06f }, extras = setOf(Extra.Hearts),
    ),
    PipMood.Embarrassed to Expression(
        EyeKind.Smile, MouthKind.Awkward, HIDDEN,
        look = Look(-0.5f, 0.6f), blush = 1f, leaves = 6f, hands = Hands.Face, extras = setOf(Extra.Sweat),
    ),
    PipMood.RestEyes to Expression(EyeKind.Closed, MouthKind.Small, HIDDEN, leaves = 2f, hands = Hands.Eyes),
    PipMood.Stretch to Expression(
        EyeKind.Closed, MouthKind.Smile, { t, s -> 2.3f + sin(t / 1100f + if (s > 0) PI_F else 0f) * 0.2f },
        leaves = -6f, tilt = { t -> sin(t / 1100f) * 0.14f }, breath = { t -> 1f + 0.035f * abs(sin(t / 1100f)) },
    ),
    PipMood.Sip to Expression(EyeKind.Smile, MouthKind.None, still(0.5f), blush = 0.6f, hands = Hands.Cup),
    PipMood.Breathe to Expression(
        EyeKind.Closed, MouthKind.Small, { t, _ -> 0.3f + 0.6f * inhale(t) },
        breath = { t -> 1f + 0.05f * inhale(t) },
    ),
    PipMood.Yawn to Expression(
        EyeKind.Closed, MouthKind.Yawn, { t, s -> if (s > 0) 2.6f + sin(t / 700f) * 0.1f else 0.3f },
        leaves = 8f, extras = setOf(Extra.Zzz),
    ),
)

fun expressionOf(mood: PipMood): Expression = EXPRESSIONS[mood] ?: EXPRESSIONS.getValue(PipMood.Idle)

/** The 20 expressions the user listed, in order. */
val GALLERY = listOf(
    PipMood.Happy, PipMood.Excited, PipMood.Laughing, PipMood.Sad, PipMood.Crying, PipMood.Angry,
    PipMood.Confused, PipMood.Surprised, PipMood.Shocked, PipMood.Thinking, PipMood.Curious, PipMood.Shy,
    PipMood.Proud, PipMood.Sleepy, PipMood.Bored, PipMood.Worried, PipMood.Playful, PipMood.Loving,
    PipMood.Embarrassed, PipMood.Celebrate,
)
