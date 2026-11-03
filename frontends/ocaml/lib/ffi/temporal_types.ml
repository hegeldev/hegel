type date =
  { year : int
  ; month : int
  ; day : int
  }

type time =
  { hour : int
  ; minute : int
  ; second : int
  ; nanosecond : int
  }

type datetime =
  { date : date
  ; time : time
  }

type instant =
  { second : int
  ; nanosecond : int
  }

type utc_offset =
  | Known of int
  | Unknown

type time_zone =
  { name : string
  ; primary_name : string
  }

type transition_kind =
  | Unique
  | Gap
  | Fold

type zoned_datetime =
  { zone : time_zone
  ; civil : datetime
  ; kind : transition_kind
  ; offset_before : utc_offset
  ; offset_after : utc_offset
  ; fold : bool
  ; offset : utc_offset
  ; instant : instant
  }

type signed_duration =
  { seconds : int
  ; nanoseconds : int
  }

type span =
  { years : int
  ; months : int
  ; weeks : int
  ; days : int
  ; hours : int
  ; minutes : int
  ; seconds : int
  ; milliseconds : int
  ; microseconds : int
  ; nanoseconds : int
  }

type precision =
  | Nanoseconds
  | Microseconds
  | Milliseconds
  | Seconds
  | Minutes
  | Hours

type granularity =
  | Hours
  | Quarter_hours
  | Minutes
  | Seconds

type weekday =
  | Monday
  | Tuesday
  | Wednesday
  | Thursday
  | Friday
  | Saturday
  | Sunday

type month =
  | January
  | February
  | March
  | April
  | May
  | June
  | July
  | August
  | September
  | October
  | November
  | December

type ambiguous =
  | Both
  | Earlier
  | Later

type span_sign =
  | Uniform
  | Mixed
  | Non_negative

type span_unit =
  | Years
  | Months
  | Weeks
  | Days
  | Hours
  | Minutes
  | Seconds
  | Milliseconds
  | Microseconds
  | Nanoseconds

type tag =
  | Fixed
  | Whole_hour
  | Fractional
  | Seconds
  | Dst
  | Dst_other
  | Multi
  | Base_change
  | Midnight
  | Spans_midnight
  | Whole_day
  | Same_offset
  | Undefined
  | Link
  | Legacy
  | Inverted_sign

type zone_info =
  { primary_name : string
  ; is_link : bool
  ; rank : int
  ; tags : tag list
  }

type zone_offset =
  { offset : utc_offset
  ; is_dst : bool
  ; abbreviation : string
  }

type zone_type =
  { offset_seconds : int
  ; is_dst : bool
  ; abbreviation : string
  }

type zone_transition =
  { instant : int
  ; local_type : zone_type
  }

type rule_day =
  | Julian of int
  | Zero_based of int
  | Month_week_day of
      { month : month
      ; week : int
      ; weekday : weekday
      }

type rule_date =
  { day : rule_day
  ; time : int
  }

type dst_rule =
  { local_type : zone_type
  ; starts : rule_date
  ; ends : rule_date
  }

type zone_rule =
  { standard : zone_type
  ; dst : dst_rule option
  }

type after =
  | Hold
  | Rule of zone_rule
  | Infer of int

type zone_data =
  { initial : zone_type
  ; transitions : zone_transition list
  ; after : after
  }
