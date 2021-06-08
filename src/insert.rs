use std::{env, process};
use serde::{Serialize, Deserialize};
use serde_json::{self as sj};
use ureq::json as json;
use base64::decode;


#[derive(Deserialize, Serialize, Debug)]
struct ICFRequestBody {
    _id: Option<String>,
    task: String,
    done: bool 
}

/* Example Raw HTTP payload from IBM Cloud Functions:
{
    "<custom_params>": <value>,
    ...
    "__ow_method": "post",
    "__ow_query": "name=Jane",
    "__ow_body": "eyJuYW1lIjoiSmFuZSJ9",
    "__ow_headers": {
    "accept": "*\/\*",
    "connection": "close",
    "content-length": "15",
    "content-type": "application/json",
    "host": "172.17.0.1",
    "user-agent": "curl/7.43.0"
    },
    "__ow_path": ""
} */
#[derive(Deserialize, Serialize)]
struct ICFRawInput {
    iam_apikey: String,
    db_url: String,
    database: String,
    __ow_body: String,
    __ow_headers: sj::Value,
    __ow_method: String,
    __ow_path: String,
    __ow_query: String
}

/* Example HTTP response from IBM Cloud IAM:
{
    "access_token": "<omitted>",
    "refresh_token": "not_supported",
    "token_type": "Bearer",
    "expires_in": 3600,
    "expiration": 1616239535,
    "scope": "ibm openid"
} */
#[derive(Deserialize, Serialize)]
struct IAMResponse {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: i32,
    expiration: i32,
    scope: String
}

/* Example HTTP response from IBM Cloudant:
{
    "offset": 0,
    "rows": [
        {
            "doc": {
                "_id": "exampleid",
                "_rev": "1-967a00dff5e02add41819138abb3284d"
            },
            "id": "exampleid",
            "key": "exampleid",
            "value": {
                "rev": "1-967a00dff5e02add41819138abb3284d"
            }
        }
    ],
    "total_rows": 1
} */
#[derive(Deserialize, Serialize, Debug)]
struct CDBResponse {
    id: String,
    ok: bool,
    rev: String
}


fn main() {
    
    // Read input arguments as a vector of Strings
    let args: Vec<String> = env::args().collect();
    println!("{:?}", &args);

    // Use serde_json to deserialize a &str into a Payload struct
    // NOTE: The `args[0]` element is traditionally the path of
    // the executable, but it can be set to arbitrary text, and
    // may not even exist. This means this property should not be 
    // relied upon for security purposes.
    let i: ICFRawInput = match sj::from_str(&args[1]) {
        Ok(res) => res,
        Err(err) => {
            // Failed to parse input into expected Rust struct
            // Return error message
            let o = json!({
                "statusCode": "200 OK",
                "body": {
                    "err": true,
                    "msg": format!("Failure parsing raw HTTP request: {}", err)
                }
            });
            // The serverless function output is pushed to stdout
            println!("{}", sj::to_string(&o).unwrap());
            // The process is killed through the OS exitcode
            process::exit(exitcode::OK)
        }
    };

    // Decode input request body from base64
    let bytes = match decode(i.__ow_body) {
        Ok(res) => res,
        Err(err) => {
            // Failed to decode base64 body
            let o = json!({
                "statusCode": "200 OK",
                "body": {
                    "err": true,
                    "msg": format!("Failure decoding base64 body: {}", err)
                }
            });
            // The serverless function output is pushed to stdout
            println!("{}", sj::to_string(&o).unwrap());
            // The process is killed through the OS exitcode
            process::exit(exitcode::OK)
        }
    };

    // Deserialize decoded bytes
    let document: ICFRequestBody = match sj::from_slice(&bytes) {
        Ok(res) => res,
        Err(err) => {
            // Failed to deserialize decoded bytes
            let o = json!({
                "statusCode": "200 OK",
                "body": {
                    "err": true,
                    "msg": format!("Failure deserializing decoded bytes: {}", err)
                }
            });
            // The serverless function output is pushed to stdout
            println!("{}", sj::to_string(&o).unwrap());
            // The process is killed through the OS exitcode
            process::exit(exitcode::OK)
        }
    };

    // Request IAM token from IBM Cloud
    /* Reference request:
        curl -X POST \
            "https://iam.cloud.ibm.com/identity/token" \
            --header 'Content-Type: application/x-www-form-urlencoded' \
            --header 'Accept: application/json' \
            --data-urlencode 'grant_type=urn:ibm:params:oauth:grant-type:apikey' \
            --data-urlencode 'apikey={api_key}'
    */
    let iam_resp = match ureq::post("https://iam.cloud.ibm.com/identity/token")
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Accept", "application/json")
        .send_form(&[
            ("apikey", &i.iam_apikey),
            ("grant_type", "urn:ibm:params:oauth:grant-type:apikey")
        ]) {
            Ok(iam_resp) => iam_resp,
            Err(_) => {
                // Failure requesting IAM token
                // Return error message
                let o = json!({
                    "statusCode": "200 OK",
                    "body": {
                        "err": true,
                        "msg": format!("Failure requesting IAM token")
                    }
                });
                // The serverless function output is pushed to stdout
                println!("{}", sj::to_string(&o).unwrap());
                // The process is killed through the OS exitcode
                process::exit(exitcode::OK)
            }
        };

    // Deserialize IAM response
    let iam_token = match iam_resp.into_json::<IAMResponse>() {
        Ok(iam_data) => iam_data.access_token,
        Err(err) => {
            // Failure deserializing IAM response
            // Return error message
            let o = json!({
                "statusCode": "200 OK",
                "body": {
                    "err": true,
                    "msg": format!("Failure deserializing IAM response: {}", err)
                }
            });
            // The serverless function output is pushed to stdout
            println!("{}", sj::to_string(&o).unwrap());
            // The process is killed through the OS exitcode
            process::exit(exitcode::OK)
        }
    };

    // Query Cloudant the database
    let uri = format!("{}/{}", &i.db_url, &i.database);
    let bearer = format!("Bearer {}", &iam_token);
    let cdb_resp = match ureq::post(&uri)
        .set("Authorization", &bearer)
        .set("Accept", "application/json")
        .send_json(json!(document)) {
            Ok(res) => res,
            Err(err) => {
                // Failure querying Cloudant
                // Return error message
                let o = json!({
                    "statusCode": "200 OK",
                    "body": {
                        "err": true,
                        "msg": format!("Failure querying Cloudant: {}", err)
                    }
                });
                // The serverless function output is pushed to stdout
                println!("{}", sj::to_string(&o).unwrap());
                // The process is killed through the OS exitcode
                process::exit(exitcode::OK)
            }
        };

    // Deserialize Cloudant response
    let cdb_data = match cdb_resp.into_json::<CDBResponse>() {
        Ok(res) => res,
        Err(err) => {
            // Failure deserializing IAM response
            // Return error message
            let o = json!({
                "statusCode": "200 OK",
                "body": {
                    "err": true,
                    "msg": format!("Failure deserializing IAM response: {}", err)
                }
            });
            // The serverless function output is pushed to stdout
            println!("{}", sj::to_string(&o).unwrap());
            // The process is killed through the OS exitcode
            process::exit(exitcode::OK)
        }
    };

    // Build output struct
    let o = json!({
        "statusCode": "200 OK",
        "body": json!({
            "err": false,
            "msg": "insert execution complete!",
            "inserted_record": cdb_data
        })
    });

    // The wsk function output is pushed to stdout
    println!("{}", sj::to_string(&o).unwrap());
    // The process is killed through the OS exitcode
    process::exit(exitcode::OK)
}
