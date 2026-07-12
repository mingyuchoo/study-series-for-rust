use std::{collections::HashMap,
          io,
          io::Write};

#[derive(Debug, Clone)]
struct Bill {
    name: String,
    amount: f64,
}

#[derive(Debug)]
struct Bills {
    items: HashMap<String, Bill>,
}

impl Bills {
    /// Self is Bills.
    /// and we can change Self to Bills in this code.
    fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    fn insert(&mut self, bill: Bill) { self.items.insert(bill.name.clone(), bill); }

    fn select_all(&self) -> Vec<Bill> {
        let mut bills = Vec::new();

        self.items.values().for_each(|bill| bills.push(bill.clone()));

        bills
    }

    fn update(&mut self, name: &str, amount: f64) -> bool {
        match self.items.get_mut(name) {
            | Some(bill) => {
                bill.amount = amount;
                true
            },
            | None => false,
        }
    }

    fn delete(&mut self, name: &str) -> bool { self.items.remove(name).is_some() }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    main_menu();

    Ok(())
}

fn main_menu() {
    fn show() {
        println!();
        println!("== Manage Bills ==");
        println!("1. View   bills");
        println!("2. Add    bill");
        println!("3. Update bill");
        println!("4. Remove bill");
        println!();
        print!("Enter menu number which you want to do: ");
        let _ = io::stdout().flush();
    }

    let mut bills = Bills::new();

    loop {
        show();

        let input = match get_input() {
            | Some(input) => input,
            | None => return,
        };

        match input.as_str() {
            | "1" => do_select_bills(&bills),
            | "2" => do_insert_bill(&mut bills),
            | "3" => do_update_bill(&mut bills),
            | "4" => do_delete_bill(&mut bills),
            // "q" | "Q" | "quit" | "Quit" => break,
            | _ => continue,
        }
    }
}

fn do_select_bills(bills: &Bills) {
    println!("-------------------------------------");

    bills
        .select_all()
        .iter()
        .for_each(|bill| println!("- name: {:<10} amount: {:>10}", bill.name, bill.amount));
}

fn do_insert_bill(bills: &mut Bills) {
    print!("Bill name: ");
    let _ = io::stdout().flush();

    let name = match get_input() {
        | Some(input) => input,
        | None => return,
    };
    let amount = match get_bill_amount() {
        | Some(amount) => amount,
        | None => return,
    };

    let bill = Bill {
        name,
        amount,
    };
    bills.insert(bill);
    println!("The bill added.");
}

fn do_update_bill(bills: &mut Bills) {
    bills.select_all().iter().for_each(|bill| println!("{:?}", bill));

    print!("Enter bill name to update: ");
    let _ = io::stdout().flush();

    let input = match get_input() {
        | Some(input) => input,
        | None => return,
    };

    let amount = match get_bill_amount() {
        | Some(amount) => amount,
        | None => return,
    };

    match bills.update(&input, amount) {
        | true => println!("The bill was updated."),
        | false => println!("The bill not found."),
    }
}

fn do_delete_bill(bills: &mut Bills) {
    bills.select_all().iter().for_each(|bill| println!("{:?}", bill));

    print!("Enter bill name to remove: ");
    let _ = io::stdout().flush();

    let input = match get_input() {
        | Some(input) => input,
        | None => return,
    };

    match bills.delete(&input) {
        | true => println!("The bill was removed."),
        | false => println!("The bill not found."),
    }
}

fn get_input() -> Option<String> {
    let mut buffer = String::new();

    while io::stdin().read_line(&mut buffer).is_err() {
        println!("Please enter your data again.");
    }

    let input = buffer.trim().to_owned();

    match input {
        | input if input.is_empty() => None,
        | _ => Some(input),
    }
}

fn get_bill_amount() -> Option<f64> {
    print!("Amount of your bill: ");
    let _ = io::stdout().flush();

    loop {
        let input: String = match get_input() {
            | Some(input) if input.is_empty() => return None,
            | Some(input) => input,
            | None => "".to_owned(),
        };

        let parsed_input: Result<f64, _> = input.parse();

        match parsed_input {
            | Ok(amount) => return Some(amount),
            | Err(_) => println!("Please enter a number."),
        }
    }
}
