package dev.hegel.generators.temporal;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.time.DayOfWeek;
import java.time.Instant;
import java.time.LocalDateTime;
import java.time.Month;
import java.time.ZoneOffset;
import java.time.temporal.ChronoUnit;
import java.time.zone.ZoneOffsetTransition;
import java.time.zone.ZoneOffsetTransitionRule;
import java.time.zone.ZoneRules;
import java.util.ArrayList;
import java.util.List;

/**
 * Translates {@code java.time} zone rules into the engine's structured zone data: the explicit
 * transitions of {@link ZoneRules#getTransitions()}, and the two {@link ZoneOffsetTransitionRule}s
 * of {@link ZoneRules#getTransitionRules()} as the yearly daylight-saving rule in force after them.
 *
 * <p>A rule's day maps onto the RFC 9636 forms: "the first weekday on or after day {@code d}" onto
 * a week of the month shifted by up to six days, "the last weekday on or before day {@code d}" onto
 * the month's last week shifted back, and a fixed day onto a Julian day (February counted from its
 * end becomes March 1 shifted back). The shift goes into the time of day, as does the conversion
 * from the rule's UTC, standard or wall time definition to the offset in force before the
 * transition, which is what the engine's rule times are read in. Rules no such form expresses, or
 * that are not one pair alternating between two offsets, are expanded into explicit transitions
 * through {@link #EXPANDED_UNTIL} instead, after which the engine infers a yearly rule as it does
 * for an oracle.
 */
final class JavaTimeZones {
    private JavaTimeZones() {}

    static final int EXPANDED_UNTIL = 2400;

    private static final int MAX_RULE_TIME = 167 * 3600;
    private static final int DAY = 86_400;
    private static final LocalDateTime REFERENCE = LocalDateTime.of(2000, 1, 1, 0, 0);

    static Libhegel.ZoneData data(ZoneRules rules) {
        List<ZoneOffsetTransition> history = rules.getTransitions();
        List<ZoneOffsetTransitionRule> yearly = rules.getTransitionRules();
        Instant first =
                history.isEmpty() ? Instant.EPOCH : history.get(0).getInstant().minusSeconds(1);
        Libhegel.ZoneType initial = type(rules, rules.getOffset(first), first);
        List<Libhegel.ZoneTransition> transitions = new ArrayList<>();
        for (ZoneOffsetTransition t : history) {
            transitions.add(transition(rules, t));
        }
        Libhegel.ZoneRuleDate none = new Libhegel.ZoneRuleDate(Abi.ZONE_RULE_DAY_JULIAN, 1, 0, 0, 0, 0);
        if (history.isEmpty() || yearly.isEmpty()) {
            return new Libhegel.ZoneData(
                    initial, transitions, Abi.ZONE_AFTER_HOLD, initial, false, initial, none, none, 0);
        }
        Libhegel.ZoneRuleDate[] dates = pair(yearly);
        if (dates == null) {
            Instant until = LocalDateTime.of(EXPANDED_UNTIL, 1, 1, 0, 0).toInstant(ZoneOffset.UTC);
            ZoneOffsetTransition next =
                    rules.nextTransition(history.get(history.size() - 1).getInstant());
            while (next.getInstant().isBefore(until)) {
                transitions.add(transition(rules, next));
                next = rules.nextTransition(next.getInstant());
            }
            return new Libhegel.ZoneData(
                    initial,
                    transitions,
                    Abi.ZONE_AFTER_INFER,
                    initial,
                    false,
                    initial,
                    none,
                    none,
                    EXPANDED_UNTIL - 1);
        }
        int s = dst(yearly.get(0)) ? 0 : 1;
        ZoneOffsetTransitionRule start = yearly.get(s);
        return new Libhegel.ZoneData(
                initial,
                transitions,
                Abi.ZONE_AFTER_RULE,
                ruleType(start, start.getOffsetBefore()),
                true,
                ruleType(start, start.getOffsetAfter()),
                dates[s],
                dates[1 - s],
                0);
    }

    /** The dates of a pair of rules alternating between two offsets, or null when they are not one or a date has no form. */
    private static Libhegel.ZoneRuleDate[] pair(List<ZoneOffsetTransitionRule> yearly) {
        if (yearly.size() != 2) {
            return null;
        }
        ZoneOffsetTransitionRule a = yearly.get(0);
        ZoneOffsetTransitionRule b = yearly.get(1);
        if (!a.getOffsetBefore().equals(b.getOffsetAfter())
                || !a.getOffsetAfter().equals(b.getOffsetBefore())) {
            return null;
        }
        Libhegel.ZoneRuleDate first = date(a);
        Libhegel.ZoneRuleDate second = date(b);
        return first == null || second == null ? null : new Libhegel.ZoneRuleDate[] {first, second};
    }

    private static boolean dst(ZoneOffsetTransitionRule rule) {
        return !rule.getOffsetAfter().equals(rule.getStandardOffset());
    }

    /** The rule's date in an RFC 9636 form whose time stays within the engine's ±167 hours, or null. */
    static Libhegel.ZoneRuleDate date(ZoneOffsetTransitionRule rule) {
        int seconds = rule.getLocalTime().toSecondOfDay()
                + (rule.isMidnightEndOfDay() ? DAY : 0)
                + (int) ChronoUnit.SECONDS.between(
                        REFERENCE,
                        rule.getTimeDefinition()
                                .createDateTime(REFERENCE, rule.getStandardOffset(), rule.getOffsetBefore()));
        Month month = rule.getMonth();
        int dom = rule.getDayOfMonthIndicator();
        DayOfWeek dow = rule.getDayOfWeek();
        int length = month.length(false);
        if (dow == null) {
            if (dom < 0 && month == Month.FEBRUARY) {
                return within(Abi.ZONE_RULE_DAY_JULIAN, 60, 0, 0, 0, seconds + dom * DAY);
            }
            int day = dom > 0 ? dom : length + 1 + dom;
            return within(Abi.ZONE_RULE_DAY_JULIAN, month.firstDayOfYear(false) - 1 + day, 0, 0, 0, seconds);
        }
        int weekday = dow.getValue() % 7;
        Libhegel.ZoneRuleDate best = null;
        int last = dom > 0 ? (month == Month.FEBRUARY ? -1 : length - 6 - dom) : -dom - 1;
        if (last >= 0) {
            best = shortest(best, weekOf(month, 5, weekday, -last, seconds));
        }
        int from = dom > 0 ? dom : (month == Month.FEBRUARY ? 0 : length + dom - 5);
        if (from >= 1 && from <= 28) {
            best = shortest(best, weekOf(month, (from - 1) / 7 + 1, weekday, (from - 1) % 7, seconds));
        }
        return best;
    }

    private static Libhegel.ZoneRuleDate weekOf(Month month, int week, int weekday, int shift, int seconds) {
        return within(
                Abi.ZONE_RULE_DAY_MONTH_WEEK_DAY,
                0,
                month.getValue(),
                week,
                Math.floorMod(weekday - shift, 7),
                seconds + shift * DAY);
    }

    private static Libhegel.ZoneRuleDate within(int kind, int day, int month, int week, int weekday, int time) {
        return Math.abs(time) <= MAX_RULE_TIME
                ? new Libhegel.ZoneRuleDate(kind, day, month, week, weekday, time)
                : null;
    }

    private static Libhegel.ZoneRuleDate shortest(Libhegel.ZoneRuleDate best, Libhegel.ZoneRuleDate candidate) {
        if (candidate == null) {
            return best;
        }
        return best == null || Math.abs(candidate.time()) < Math.abs(best.time()) ? candidate : best;
    }

    private static Libhegel.ZoneType ruleType(ZoneOffsetTransitionRule rule, ZoneOffset offset) {
        return new Libhegel.ZoneType(
                offset.getTotalSeconds(), !offset.equals(rule.getStandardOffset()), abbreviation(offset));
    }

    private static Libhegel.ZoneTransition transition(ZoneRules rules, ZoneOffsetTransition t) {
        return new Libhegel.ZoneTransition(t.toEpochSecond(), type(rules, t.getOffsetAfter(), t.getInstant()));
    }

    private static Libhegel.ZoneType type(ZoneRules rules, ZoneOffset offset, Instant at) {
        return new Libhegel.ZoneType(offset.getTotalSeconds(), rules.isDaylightSavings(at), abbreviation(offset));
    }

    /** {@code java.time} has no abbreviations, so a type is named by its offset, as tzdb names zones without one. */
    static String abbreviation(ZoneOffset offset) {
        return offset.getTotalSeconds() == 0 ? "UTC" : offset.getId().replace(":", "");
    }
}
