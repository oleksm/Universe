//! CPU-only independent reader audit of bounded canonical-face overlay artifacts.
use glam::DVec3;
use serde_json::{Value,json};
use universe_world::worlds::{pgs::{Surface,Water},sha256};
fn vec(v:&Value)->DVec3 { DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap()) }
fn visible(s:&Surface,d:DVec3)->Result<f64,String>{let q=s.query(d)?;Ok(match q.water {Water::Wet{level_m,..}=>level_m,_=>q.height_m})}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<_>=std::env::args().skip(1).collect();
 if args.len()!=5{return Err("surface.pgs geometry-dir features.json plan.json report.json".into())}
 let bytes=std::fs::read(&args[0])?;let clock=std::time::Instant::now();let surface=Surface::read(&bytes)?;let decode_ms=clock.elapsed().as_secs_f64()*1000.;
 let features:Value=serde_json::from_slice(&std::fs::read(&args[2])?)?;
 let plan:Value=serde_json::from_slice(&std::fs::read(&args[3])?)?;let radius=plan["radius_m"].as_f64().unwrap();let max_samples=plan["max_sample_queries_per_pair"].as_u64().unwrap();
 if sha256(&bytes)!=plan["source_sha256"].as_str().unwrap(){return Err("source hash mismatch".into())}
 let mut files=std::fs::read_dir(&args[1])?.map(|f|f.unwrap().path()).filter(|p|p.extension().is_some_and(|e|e=="json")).collect::<Vec<_>>();files.sort();let mut results=vec![];
 let weights=[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[0.5,0.5,0.],[0.5,0.,0.5],[0.,0.5,0.5],[0.1,0.9,0.],[0.,0.1,0.9],[0.9,0.,0.1],[1./3.;3],[0.6,0.2,0.2],[0.2,0.6,0.2],[0.2,0.2,0.6]];
 for file in files {
  let data=std::fs::read(&file)?;let g:Value=serde_json::from_slice(&data)?;let Some(tiles)=g["tiles"].as_array() else {continue};let timer=std::time::Instant::now();
  let mut triangles=vec![];let mut count=0u64;let mut max=0f64;let mut sum=0f64;let mut worst=Value::Null;let mut owner_differences=0u64;
  for tile in tiles {
   let origin=vec(&tile["origin"]);let vertices=tile["vertices"].as_array().unwrap().iter().map(|v|origin+vec(v).as_vec3().as_dvec3()).collect::<Vec<_>>();
   for (i,ids) in tile["triangles"].as_array().unwrap().iter().enumerate(){
    let tri=[0,1,2].map(|j|vertices[ids[j].as_u64().unwrap() as usize]);triangles.push(tri);
    for (sample,w) in weights.iter().enumerate(){
     count+=1;if count>max_samples{return Err("sample cap".into())}
     let p=tri[0]*w[0]+tri[1]*w[1]+tri[2]*w[2];let d=p.normalize();let query=surface.query(d)?;let expected=match query.water{Water::Wet{level_m,..}=>level_m,_=>query.height_m};let error=p.length()-radius-expected;
     if sample==9 && query.face as u64!=tile["provenance"][i][0].as_u64().unwrap(){owner_differences+=1}
     sum+=error*error;if error.abs()>max{max=error.abs();worst=json!({"error_m":error,"direction":d.to_array(),"source_face":tile["provenance"][i][0],"query_face":query.face,"sample":sample,"water":format!("{:?}",query.water)});}
    }
   }
  }
  let mut frozen=vec![];
  if let Some(feature)=features["features"].as_array().unwrap().iter().find(|f|f["id"]==g["feature"]){
   for q in feature["queries"].as_array().unwrap(){
    let d=vec(&q["unit_direction"]).normalize();let expected=visible(&surface,d)?;let mut hits=vec![];
    for &[a,b,c] in &triangles{
     let n=(b-a).cross(c-a);let den=n.dot(d);if den.abs()<1e-20{continue}let distance=n.dot(a)/den;if distance<=0.{continue}let p=d*distance;let nn=n.length_squared();let v=(p-a).cross(c-a).dot(n)/nn;let w=(b-a).cross(p-a).dot(n)/nn;
     if v>=-1e-7&&w>=-1e-7&&v+w<=1.+1e-7{hits.push(distance-radius-expected)}
    }
    frozen.push(json!({"label":q["label"],"hits":hits,"max_abs_error_m":hits.iter().map(|x|x.abs()).reduce(f64::max)}));
   }
  }
  results.push(json!({"feature":g["feature"],"geometry_sha256":sha256(&data),"sample_count":count,"max_error_m":max,"rms_error_m":(sum/count as f64).sqrt(),"worst":worst,"centroid_owner_differences":owner_differences,"frozen":frozen,"audit_ms":timer.elapsed().as_secs_f64()*1000.,"seam_max_m":g["seam_max_m"],"seam_points":g["seam_points"]}));
 }
 let result=json!({"source_sha256":sha256(&bytes),"decode_ms":decode_ms,"scope":"f32 tile-local vertices; exact Rust canonical query against physical triangle vertices/edge/interior samples; finite samples, no GPU/morph/continuous bound","results":results});
 std::fs::write(&args[4],serde_json::to_vec_pretty(&result)?)?;Ok(())
}
