let unbounded_magnitude = Z.shift_left Z.one 127

let to_bytes n =
  let length = (Z.numbits n / 8) + 1 in
  let unsigned = if Z.sign n < 0 then Z.add n (Z.shift_left Z.one (8 * length)) else n in
  let bits = Z.to_bits unsigned in
  if String.length bits >= length
  then String.sub bits 0 length
  else bits ^ String.make (length - String.length bits) '\000'
;;

let of_bytes s =
  let unsigned = Z.of_bits s in
  if Char.code s.[String.length s - 1] >= 0x80
  then Z.sub unsigned (Z.shift_left Z.one (8 * String.length s))
  else unsigned
;;

let integers ?min_value ?max_value () =
  let lo =
    match min_value, max_value with
    | Some lo, _ -> lo
    | None, None -> Z.neg unbounded_magnitude
    | None, Some hi ->
      Z.min (Z.neg unbounded_magnitude) (Z.succ (Z.sub hi unbounded_magnitude))
  in
  let hi =
    match max_value, min_value with
    | Some hi, _ -> hi
    | None, None -> Z.pred unbounded_magnitude
    | None, Some lo ->
      Z.max (Z.pred unbounded_magnitude) (Z.pred (Z.add lo unbounded_magnitude))
  in
  if Z.gt lo hi
  then
    raise
      (Hegel.Usage_error
         (Printf.sprintf
            "Cannot have max_value=%s < min_value=%s"
            (Z.to_string hi)
            (Z.to_string lo)));
  let min_value = to_bytes lo
  and max_value = to_bytes hi in
  Hegel.Generators.leaf
    ~name:"integers"
    ~draw:(fun tc ->
      of_bytes (Hegel.Internal.generate_integer_big tc ~min_value ~max_value))
    ~print:(Hegel.Printer.of_to_string Z.to_string)
;;
