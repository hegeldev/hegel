let%hegel_main workload tc = Bank.run tc (Bank.workload_from_env_or_draw tc)
[@@settings { (Hegel.Settings.default ()) with show_statistics = true }]
;;

let () = workload ()
