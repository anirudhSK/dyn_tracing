extern crate dyntracing;
extern crate handlebars;
extern crate serde;

use dyntracing::{code_gen, lexer, parser, tree_fold::TreeFold};
use handlebars::Handlebars;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

fn main() {
    let template_path = Path::new("filter.cc.handlebars");
    let display = template_path.display();
    let mut template_file = match File::open(&template_path) {
        Err(msg) => panic!("Failed to open {}: {}", display, msg),
        Ok(file) => file,
    };

    let mut template_str = String::new();
    match template_file.read_to_string(&mut template_str) {
        Err(msg) => panic!("Failed to read {}: {}", display, msg),
        Ok(_) => println!("Successfully read {}", display),
    }

    let query = "MATCH a-->b : x, b-->c : y, a-->d: z, \
                    WHERE a.service_name == productpagev1, \
                            b.service_name == reviewsv2, \
                            c.service_name == ratingsv1, \
                            d.service_name == detailsv1, \
                    RETURN aggr_func,";
    let tokens = lexer::get_tokens(query);
    let mut token_iter = tokens.iter().peekable();
    let parse_tree = parser::parse_prog(&mut token_iter);

    let mut code_gen = code_gen::CodeGen::new();

    code_gen.config.udf_table.insert(
        "aggr_func",
        code_gen::Udf {
            udf_type: code_gen::UdfType::Aggregation,
            id: "aggr_func",
            func_impl: r#"
class aggr_func : public user_func<int> {
public:
    int operator()(const trace_graph_t &graph) {
        num_vertices += graph.num_vertices();
        return num_vertices;
    }

private:
    int num_vertices = 0;
};"#,
            return_type: "int",
            ..Default::default()
        },
    );

    code_gen.config.udf_table.insert(
        "sum_aggr",
        code_gen::Udf {
            udf_type: code_gen::UdfType::Aggregation,
            id: "sum_aggr",
            func_impl: r#"
            "#,
            return_type: "int",
            ..Default::default()
        },
    );

    code_gen.root_id = "productpagev1";
    code_gen.visit_prog(&parse_tree);

    let handlebars = Handlebars::new();

    let output = handlebars
        .render_template(&template_str, &code_gen)
        .expect("handlebar render failed");

    let mut file = File::create("./wasm/filter.cc").expect("file create failed.");
    file.write_all(output.as_bytes()).expect("write failed");
}
