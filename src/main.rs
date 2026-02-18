mod models;
mod grafana_client;
mod garmin_client;
mod coaching;

use crate::garmin_client::GarminClient;
use crate::coaching::Coach;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Fitness Coach...");

    // 1. Fetch History from InfluxDB (via Grafana)
    // Default to the known working IP/Token if not set
    let grafana_url = std::env::var("GRAFANA_URL").unwrap_or_else(|_| "http://10.0.15.4:3000".to_string());
    let grafana_token = std::env::var("GRAFANA_TOKEN").unwrap_or_else(|_| "***REMOVED***".to_string());
    
    // Initialize components
    // println!("Connecting to Grafana at {}...", grafana_url);
    let client = crate::grafana_client::GrafanaClient::new(grafana_url.clone(), grafana_token.clone());
    let garmin_client = GarminClient::new();
    let coach = Coach::new();
    
    let mut activities = Vec::new();

    // 1. Fetch High-Level Data from InfluxDB
    println!("Connecting to Grafana at {}...", grafana_url);
    if let Err(e) = client.test_connection().await {
        eprintln!("Warning: Could not connect to Grafana: {}", e);
    } else {
        println!("Successfully connected to Grafana!");
        println!("Fetching summary activities from InfluxDB...");
        match client.get_recent_activities(30).await {
            Ok(acts) => {
                println!("Found {} summary activities.", acts.len());
                activities = acts;
            },
            Err(e) => eprintln!("Failed to fetch activities: {}", e),
        }
    }

    // 2. Fetch Detailed Data from Garmin Connect (Python Bridge)
    println!("\nFetching detailed stats from Garmin Connect...");
    let (detailed_activities, active_plans, user_profile, max_metrics) = match garmin_client.fetch_data() {
        Ok(response) => {
            println!("Found {} detailed activities and {} active plans.", response.activities.len(), response.plans.len());
            (response.activities, response.plans, response.user_profile, response.max_metrics)
        },
        Err(e) => {
            eprintln!("Failed to fetch detailed Garmin data: {}", e);
            (Vec::new(), Vec::new(), None, None)
        }
    };

    // 3. Define Context & Goals
    let context = crate::coaching::CoachContext {
        goals: vec![
            "Improve Marathon Time (Sub 4h)".to_string(),
            "Maintain Upper Body Strength (Hypertrophy)".to_string(),
            "Increase VO2Max".to_string(),
        ],
        constraints: vec![
            "Run: Availability 7 days/week (Follow Garmin Coach prescriptions)".to_string(),
            "Strength: 3x week (Hypertrophy/Maintenance)".to_string(),
            "Recovery: Listen to Body Battery".to_string(),
        ],
        available_equipment: vec![
            "Dumbbells (Fixed Pairs): 2.5kg, 5kg, 7.5kg, 10kg".to_string(),
            "Dumbbells (Adjustable Pair): 4kg, 8kg, 12kg, 16kg, 20kg, 24kg".to_string(),
            "Kettlebell: 12kg".to_string(),
            "Pull-up Rod".to_string(),
            "Weight Bench".to_string(),
            "Sit-up Cushion".to_string(),
            "Rowing Machine".to_string(),
            "Skipping Rope".to_string(),
        ]
    };

    // Generate Brief
    println!("\nGenerating Coach Brief...");
    let brief = coach.generate_brief(
        &activities, 
        &detailed_activities, 
        &active_plans,
        &user_profile,
        &max_metrics,
        &context
    );

    println!("===================================================");
    println!("{}", brief);
    println!("===================================================");

    Ok(())
}
