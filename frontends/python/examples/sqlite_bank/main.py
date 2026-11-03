import bank

import hegel


@hegel.main(show_statistics=True)
def workload(tc):
    bank.run(tc, bank.bank_workload(tc))


if __name__ == "__main__":
    workload()
