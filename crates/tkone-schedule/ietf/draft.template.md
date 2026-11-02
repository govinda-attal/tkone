---
title: "The tkone Schedule Format: Recurring Dates and Times for Business Calendars"
abbrev: "tkone Schedule Format"
docname: draft-attal-tkone-schedule-00
category: info
ipr: trust200902
submissiontype: IETF
stand_alone: yes
v: 3
keyword:
  - schedule
  - recurrence
  - business day
  - calendar
author:
  - fullname: Govinda Attal
    # Add an email address before submitting.
normative:
  RFC2119:
  RFC8174:
  RFC5234:
  RFC7405:
  RFC3339:
  RFC9557:
informative:
  RFC5545:
  RFC6557:
  TZDB:
    title: "Time Zone Database"
    target: https://www.iana.org/time-zones
    author:
      - org: IANA
  CRON:
    title: "crontab - schedule periodic background work"
    target: https://pubs.opengroup.org/onlinepubs/9799919799/utilities/crontab.html
    author:
      - org: The Open Group
  TKONE:
    title: "tkone-schedule: reference implementation"
    target: https://github.com/govinda-attal/tkone

--- abstract

This document specifies the tkone schedule format, a compact text notation for recurring dates
and times. A schedule fits on one line and can express rules found in business calendars,
such as "the 31st of every month, or the last day when the month is shorter, moved to the next
business day", "the third business day of each month", "every other Tuesday and Thursday" and
"36 monthly payments". Each occurrence records both the date the rule produced and the date
after a business-day adjustment. A schedule can carry its own start, end, time zone and number
of occurrences.

--- middle

# Introduction

Common recurrence notations were designed for other purposes. Cron {{CRON}} schedules jobs on
a single machine: it has no month end, no business days, and its steps restart at every field
boundary. The iCalendar recurrence rule {{RFC5545}} is expressive, but it has no business days
or holidays, it drops short months rather than falling back to the last day, and an occurrence
has only one date.

Business schedules need these things. A payment due on the 31st moves to the next business day
in shorter months; interest accrues to the scheduled date while cash moves on the adjusted one;
a report runs two business days before month end; a rota runs four days on and four days off.

The tkone schedule format states such rules in one short line of text:

~~~ text
YY-1M-31L~NB          the 31st of every month (or the last day),
                      then the next business day
YY-MM-L~2P            two business days before the last day
                      of every month
YY-MM-BD#3            the third business day of every month
YY-MM-[TUE,THU]/2W    Tuesday and Thursday, every other week
YY-MM-01~MF @2025-04-01 x36
                      36 monthly payments from April 2025,
                      Modified Following
~~~

This document defines the syntax ({{grammar}}), the inputs that are well-formed but invalid
({{validity-rules}}), and the occurrences a schedule produces ({{evaluation-rules}}).
{{conformance-examples}} lists accepted and rejected inputs. A reference implementation is
available {{TKONE}}.

What a business day is depends on a holiday calendar. A schedule can name its calendar
(`[cal=GBLO]`), so that the text alone fixes its dates; the calendars themselves are not part
of the format. A schedule that names none is evaluated with a calendar supplied alongside it,
so that the same text can be used for different markets.

# Conventions and Definitions

{::boilerplate bcp14-tagged}

Schedule:
: A text in the format defined by this document.

Occurrence:
: One result of a schedule. It has an *actual* time, the time the rule produced, and an
  *observed* time, the time after any business-day adjustment.

Business day:
: A day that the holiday calendar in use treats as a working day. A *weekday* is Monday to
  Friday, whatever the calendar says.

Start:
: The time a schedule is counted from. It is written in the schedule (`@`) or supplied by the
  system that evaluates it.

An implementation MUST reject a schedule that does not match the grammar in {{grammar}}, or
that breaks a rule in {{validity-rules}}. An implementation MUST produce the occurrences that
{{evaluation-rules}} describes.

<!-- include: GRAMMAR.md -->

# Security Considerations

Schedules are often supplied by users, so implementations need to treat them as untrusted
input.

A schedule can describe an unbounded number of occurrences, some of them very close together
(`HH:MM:SS` occurs every second). An implementation that collects occurrences SHOULD bound how
many it produces, or require an end or a count.

Some valid schedules never produce an occurrence, such as the 30th of February, or a schedule
whose exclusion removes every date. An implementation MUST NOT search for an occurrence
indefinitely; it SHOULD give up after a bounded search and report an error.

Lists and ranges expand into sets of values. {{validity-rules}} caps a date range at 10,000
values; an implementation SHOULD also bound the total size of the sets it builds.

The occurrences of a schedule depend on data outside it: the time zone database {{TZDB}}
{{RFC6557}} and the holiday calendar. Two systems that evaluate the same schedule with different
data can produce different occurrences. Systems that must agree, for example on a payment date,
need to agree on that data too, and to protect its integrity. Naming the calendar in the
schedule (`[cal=…]`) makes a mismatch detectable, but two systems can still hold different
versions of the same calendar.

# IANA Considerations

This document has no IANA actions.

--- back

# Collected ABNF {#collected-abnf}

This appendix collects the ABNF of {{grammar}} in one place. `ALPHA`, `DIGIT` and `SP` are
defined in {{RFC5234}}, Appendix B.1.

<!-- include: abnf -->

# Acknowledgments
{:numbered="false"}

The format grew out of the scheduling needs of the tkone project.
