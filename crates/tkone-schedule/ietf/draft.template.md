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

This document specifies the tkone schedule format, a compact text notation for a recurrence
rule of dates and times. A schedule is a single rule that fits on one line and can express
rules found in business calendars, such as "the 31st of every month, or the last day when the
month is shorter, moved to the next business day", "the third business day of each month",
"every other Tuesday and Thursday" and "the Monday nearest 29 January". Each occurrence records
both the date the rule produced and the date after an adjustment. The context a rule is
evaluated in, such as its start, time zone, holiday calendar and bounds, is supplied by the
application and is outside the scope of this document.

--- middle

# Introduction

Common recurrence notations were designed for other purposes. Cron {{CRON}} schedules jobs on
a single machine: it has no month end, no business days, and its steps restart at every field
boundary. The iCalendar recurrence rule {{RFC5545}} is expressive, but it has no business days
or holidays, it drops short months rather than falling back to the last day, and an occurrence
has only one date.

Business schedules need these things. A payment due on the 31st moves to the next business day
in shorter months; a statement is dated the scheduled day while cash moves on the adjusted one;
a report runs two business days before month end; a rota runs four days on and four days off.

The tkone schedule format states such rules in one short line of text:

~~~ text
YY-1M-31L~NB          the 31st of every month (or the last day),
                      then the next business day
YY-MM-L~2P            two business days before the last day
                      of every month
YY-MM-BD#3            the third business day of every month
YY-MM-[TUE,THU]/2W    Tuesday and Thursday, every other week
YY-05-24~PMON         the Monday on or before 24 May
~~~

This document defines the syntax ({{grammar}}), the inputs that are well-formed but invalid
({{validity-rules}}), and the occurrences a schedule produces ({{evaluation-rules}}).
{{conformance-examples}} lists accepted and rejected inputs. A reference implementation is
available {{TKONE}}.

The format schedules operations: when payments run, jobs fire and reports are due. It does not
define the economics of financial instruments (calculation periods, stubs, fixing offsets, day
counts), for which other standards exist.

A schedule describes one recurrence rule and nothing around it. The start it is counted from,
the time zone it is read in, the holiday calendar that decides what a business day is, an end
or a number of occurrences, the combination of several rules, and dates to leave out are
supplied by the application that evaluates it, for example in a separate schedule document.
This document does not define how. Keeping them apart lets the same text be used in different
time zones and for different markets.

# Conventions and Definitions

{::boilerplate bcp14-tagged}

Schedule:
: A text in the format defined by this document.

Occurrence:
: One result of a schedule. It has an *actual* time, the time the rule produced, and an
  *observed* time, the time after any adjustment.

Business day:
: A day that the holiday calendar in use treats as a working day. A *weekday* is Monday to
  Friday, whatever the calendar says.

Start:
: The time a schedule is counted from, supplied by the system that evaluates it.

An implementation MUST reject a schedule that does not match the grammar in {{grammar}}, or
that breaks a rule in {{validity-rules}}. An implementation MUST produce the occurrences that
{{evaluation-rules}} describes.

<!-- include: GRAMMAR.md -->

# Security Considerations

Schedules are often supplied by users, so implementations need to treat them as untrusted
input.

A schedule can describe an unbounded number of occurrences, some of them very close together
(`HH:MM:SS` occurs every second). An implementation that collects occurrences SHOULD bound how
many it produces, or require an end or a count from the application.

Some valid schedules never produce an occurrence, such as the 30th of February. An
implementation MUST NOT search for an occurrence indefinitely; it SHOULD give up after a bounded search and report an error.

Lists and ranges expand into sets of values. {{validity-rules}} caps each range in a date spec at
10,000 values; an implementation SHOULD also bound the total size of the sets it builds.

The occurrences of a schedule depend on data outside it: the time zone database {{TZDB}}
{{RFC6557}} and the holiday calendar. Two systems that evaluate the same schedule with different
data can produce different occurrences. Systems that must agree, for example on a payment date,
need to agree on that data, and on the calendar and time zone each schedule is evaluated with,
and to protect the data's integrity.

# IANA Considerations

This document has no IANA actions.

--- back

# Collected ABNF {#collected-abnf}

This appendix collects the ABNF of {{grammar}} in one place. `DIGIT` and `SP` are defined in {{RFC5234}}, Appendix B.1.

<!-- include: abnf -->

# Acknowledgments
{:numbered="false"}

The format grew out of the scheduling needs of the tkone project.
