module Date = struct
  type t =
    { year : int
    ; month : int
    ; day : int
    }
end

module Time = struct
  type t =
    { hour : int
    ; minute : int
    ; second : int
    ; nanosecond : int
    }
end

module Datetime = struct
  type t =
    { date : Date.t
    ; time : Time.t
    }
end

module Instant = struct
  type t =
    { second : int
    ; nanosecond : int
    }
end

module Duration = struct
  type t =
    { seconds : int
    ; nanoseconds : int
    }
end

module Span = struct
  type t =
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
end

module Offset = struct
  type t =
    { seconds : int
    ; known : bool
    }
end

module Zoned = struct
  type t =
    { civil : Datetime.t
    ; offset_seconds : int
    ; offset_known : bool
    ; fold : bool
    ; kind : int
    ; offset_before : int
    ; offset_after : int
    ; timestamp : Instant.t
    }
end

module Resolution = struct
  type t =
    { kind : int
    ; offset_before : int
    ; offset_after : int
    ; offset_seconds : int
    ; offset_known : bool
    ; timestamp : Instant.t
    }
end

module Zone_offset = struct
  type t =
    { offset_seconds : int
    ; offset_known : bool
    ; is_dst : bool
    ; abbreviation : string
    }
end

module Zone_info = struct
  type t =
    { known : bool
    ; primary_name : string
    ; is_link : bool
    ; rank : int
    ; tags : int
    }
end

module Zone_type = struct
  type t =
    { offset_seconds : int
    ; is_dst : bool
    ; abbreviation : string
    }
end

module Zone_transition = struct
  type t =
    { instant : int
    ; local_type : Zone_type.t
    }
end

module Rule_date = struct
  type t =
    { kind : int
    ; day : int
    ; month : int
    ; week : int
    ; weekday : int
    ; time : int
    }
end

module Zone_data = struct
  type t =
    { initial : Zone_type.t
    ; transitions : Zone_transition.t list
    ; after : int
    ; standard : Zone_type.t
    ; has_dst : bool
    ; dst : Zone_type.t
    ; dst_start : Rule_date.t
    ; dst_end : Rule_date.t
    ; last_year : int
    }
end
