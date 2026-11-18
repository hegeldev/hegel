type 'a t = 'a iarray

let init n f = Stdlib_stable.Iarray.init n f

external get : 'a t -> int -> 'a @@ portable = "%array_safe_get"
